//! [`PageApi`]: the page API on the host's side (RB-020-08, `docs/sync-protocol.md`, "Page API
//! and message bus"). Per view it registers the main-world script - the bundle of
//! `agent/src/page-api/` wrapped in the view's [`PageState`] - in the view's page session, and
//! registers it again when the state changes, so every new document starts with the current
//! `window.__rbrowser`; the document that runs gets the change from its agent (`page` command,
//! `rbrowser:viewchange`). It holds the session overrides of the toggle commands, the bus's rate
//! limiter per view and the engine's user agent for the suffix (`ua.rs`). The tasks that feed it
//! are `page_tasks.rs`.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use rb_config::ProjectConfig;
use rb_engine::ViewId;
use rb_sync::{
    AgentCommand, BusError, BusMessage, BusPayload, BusSettings, CommandResult, PageState,
    RateLimiter,
};
use serde_json::json;
use tokio::sync::OnceCell;

use super::SyncAgents;
use super::page_state::{Overrides, PageSwitches, Switch, main_world_source};
use super::ua::{EngineUserAgent, engine_user_agent};
use crate::cdp::{CdpRouter, lock};

/// The main-world bundle (`agent/dist/rb-page-api.js`), copied by `build.rs`; empty when
/// `agent/` was not built.
pub const PAGE_API_SOURCE: &str = include_str!(concat!(env!("OUT_DIR"), "/rb-page-api.js"));

/// The page API of one view.
#[derive(Default)]
struct ViewPage {
    /// The state last registered and sent.
    state: Option<PageState>,
    /// Whether the main-world script is meant to be there.
    api: bool,
    /// The view's page session.
    session: Option<String>,
    /// The main-world script's identifier in `session`.
    script: Option<String>,
    /// The bus's rate limiter of the view.
    limiter: Option<RateLimiter>,
}

#[derive(Default)]
struct Inner {
    overrides: Overrides,
    views: HashMap<ViewId, ViewPage>,
}

/// The page API of every view.
pub struct PageApi {
    cdp: Arc<CdpRouter>,
    agents: Arc<SyncAgents>,
    bundle: &'static str,
    inner: Mutex<Inner>,
    /// Serialises the registrations, so a later state never loses against an earlier one.
    register: tokio::sync::Mutex<()>,
    /// The engine's own user agent, read once (`Browser.getVersion`).
    engine: OnceCell<Option<EngineUserAgent>>,
    epoch: Instant,
}

impl PageApi {
    /// The page API over `cdp` and the views' `agents`, with the embedded bundle.
    pub fn new(cdp: Arc<CdpRouter>, agents: Arc<SyncAgents>) -> Arc<Self> {
        Self::with_bundle(cdp, agents, PAGE_API_SOURCE)
    }

    pub(crate) fn with_bundle(
        cdp: Arc<CdpRouter>,
        agents: Arc<SyncAgents>,
        bundle: &'static str,
    ) -> Arc<Self> {
        Arc::new(Self {
            cdp,
            agents,
            bundle,
            inner: Mutex::new(Inner::default()),
            register: tokio::sync::Mutex::new(()),
            engine: OnceCell::new(),
            epoch: Instant::now(),
        })
    }

    /// The switches of `config` under the toggles' overrides.
    pub fn switches(&self, config: &ProjectConfig) -> PageSwitches {
        lock(&self.inner).overrides.apply(config)
    }

    /// Flips `switch` for the session; returns its new value.
    pub fn toggle(&self, switch: Switch, config: &ProjectConfig) -> bool {
        lock(&self.inner).overrides.toggle(switch, config)
    }

    /// `view` has a new page `session` (an attach): the main-world script with `state` goes
    /// into it, before the first document loads.
    pub async fn attach(&self, view: ViewId, session: &str, state: PageState, api: bool) {
        let _serial = self.register.lock().await;
        {
            let mut inner = lock(&self.inner);
            let page = inner.views.entry(view).or_default();
            page.session = Some(session.to_owned());
            // The old script went with the old session.
            page.script = None;
            page.state = Some(state.clone());
            page.api = api;
        }
        let script = self.register_script(session, None, &state, api).await;
        self.keep_script(view, session, script);
    }

    /// `view`'s state now: when it changed, the script is registered again and the agent of
    /// the running document gets it.
    pub async fn update(&self, view: ViewId, state: PageState, api: bool) {
        let _serial = self.register.lock().await;
        let (session, previous) = {
            let mut inner = lock(&self.inner);
            let page = inner.views.entry(view).or_default();
            if page.state.as_ref() == Some(&state) && page.api == api {
                return;
            }
            page.state = Some(state.clone());
            page.api = api;
            (page.session.clone(), page.script.take())
        };
        if let Some(session) = session {
            let script = self.register_script(&session, previous, &state, api).await;
            self.keep_script(view, &session, script);
        }
        self.send_state(view, state).await;
    }

    /// The agent of `view`'s top frame started in a new document: it gets the state.
    pub async fn on_ready(&self, view: ViewId) {
        let state = lock(&self.inner)
            .views
            .get(&view)
            .and_then(|page| page.state.clone());
        if let Some(state) = state {
            self.send_state(view, state).await;
        }
    }

    /// The view is gone.
    pub fn forget(&self, view: ViewId) {
        lock(&self.inner).views.remove(&view);
    }

    /// Whether the core relays `message` of `view`: [`rb_sync::bus::check_message`], then the
    /// view's rate limit. The payload comes back serialised once, for the deliveries.
    pub fn accept(
        &self,
        view: ViewId,
        message: &BusMessage,
        settings: &BusSettings,
    ) -> Result<BusPayload, BusError> {
        let payload = rb_sync::bus::check_message(message, settings)?;
        let now = u64::try_from(self.epoch.elapsed().as_millis()).unwrap_or(u64::MAX);
        let rate = settings.clamped().max_per_second;
        let mut inner = lock(&self.inner);
        let page = inner.views.entry(view).or_default();
        let limiter = page
            .limiter
            .get_or_insert_with(|| RateLimiter::new(rate, now));
        limiter.set_rate(rate, now);
        if limiter.take(now) {
            Ok(payload)
        } else {
            Err(BusError::RateLimited)
        }
    }

    /// Hands a bus message to the page of each of `targets`; returns those whose agent took it
    /// (a page of another origin, with the bus off or without an agent does not). The payload's
    /// bytes go into every view's command as they are (RB-080-27).
    pub async fn deliver(
        &self,
        targets: &[ViewId],
        from: Option<ViewId>,
        origin: Option<&str>,
        r#type: &str,
        payload: &BusPayload,
    ) -> Vec<ViewId> {
        let command = AgentCommand::Bus {
            from: from.map(|view| view.0),
            origin: origin.map(str::to_owned),
            r#type: r#type.to_owned(),
            payload: payload.clone(),
        };
        let mut delivered = Vec::new();
        for &target in targets {
            match self.agents.command(target, &command).await {
                Ok(CommandResult::Done) => delivered.push(target),
                Ok(other) => tracing::debug!(view = %target, ?other, "bus message not delivered"),
                Err(error) => tracing::debug!(view = %target, %error, "bus message not delivered"),
            }
        }
        delivered
    }

    /// The engine's own user agent (`Browser.getVersion`), read on first use; `None` when the
    /// browser does not say.
    pub async fn engine_user_agent(&self) -> Option<EngineUserAgent> {
        self.engine
            .get_or_init(|| async {
                match self.cdp.call("Browser.getVersion", json!({}), None).await {
                    Ok(answer) => engine_user_agent(&answer),
                    Err(error) => {
                        tracing::warn!(%error, "the engine's user agent is unknown; no rBrowser suffix without an emulated one");
                        None
                    }
                }
            })
            .await
            .clone()
    }

    async fn send_state(&self, view: ViewId, state: PageState) {
        let command = AgentCommand::Page { page: state };
        if let Err(error) = self.agents.command(view, &command).await {
            // No agent yet: it gets the state when it says ready.
            tracing::debug!(%view, %error, "page state not sent");
        }
    }

    /// Removes `previous` and, with `api`, registers the script for `state`; its identifier.
    async fn register_script(
        &self,
        session: &str,
        previous: Option<String>,
        state: &PageState,
        api: bool,
    ) -> Option<String> {
        if let Some(identifier) = previous {
            let params = json!({ "identifier": identifier });
            if let Err(error) = self
                .cdp
                .call(
                    "Page.removeScriptToEvaluateOnNewDocument",
                    params,
                    Some(session),
                )
                .await
            {
                tracing::debug!(%error, "the page API script was not removed");
            }
        }
        if !api {
            return None;
        }
        let Some(source) = main_world_source(self.bundle, state) else {
            tracing::warn!(
                "the page API is not built (agent/: npm run build); no window.__rbrowser"
            );
            return None;
        };
        // No world name: the page's main world. At once in the running document too, so
        // switching the API on shows it without a reload.
        let params = json!({ "source": source, "runImmediately": true });
        match self
            .cdp
            .call(
                "Page.addScriptToEvaluateOnNewDocument",
                params,
                Some(session),
            )
            .await
        {
            Ok(answer) => answer["identifier"].as_str().map(str::to_owned),
            Err(error) => {
                tracing::warn!(%error, "the page API script was not registered");
                None
            }
        }
    }

    /// Keeps `script` as the view's, unless the session changed in the meantime.
    fn keep_script(&self, view: ViewId, session: &str, script: Option<String>) {
        let mut inner = lock(&self.inner);
        if let Some(page) = inner.views.get_mut(&view)
            && page.session.as_deref() == Some(session)
        {
            page.script = script;
        }
    }
}

#[cfg(test)]
#[path = "page_tests.rs"]
mod tests;
