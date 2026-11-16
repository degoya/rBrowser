//! [`PluginPages`]: the plugins' page scripts in every view (RB-060-12). Each plugin gets an
//! isolated world of its own in each view's page session - `Runtime.addBinding` with the
//! world's name, `Page.addScriptToEvaluateOnNewDocument` with `worldName` and `runImmediately` -
//! like the sync agent (`crate::sync_agent`), and again after every attach (a reconnect, a
//! renderer that came back). The page sees neither the script nor the binding.
//!
//! A binding call is untrusted input: its session must be a view's, its context the plugin's
//! world, its payload at most 64 KiB (`rb_plugin_host::parse_page_payload`). A request goes to
//! the [`Inbox`] and its answer back into the calling context; an event goes to the panels; a
//! logged line (`l:`, RB-080-66) to the [`Inbox`] for rBrowser's log and console.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, Weak};

use async_trait::async_trait;
use rb_engine::{EngineError, ViewId};
use rb_plugin_host::{
    PageBridge, PageInput, PageLog, PageRefusal, parse_page_payload, receiver_call, reply_message,
    stop_message,
};
use rb_plugin_types::codes;
use serde_json::{Value, json};
use tokio::runtime::Handle;
use tokio::sync::broadcast::error::RecvError;

use crate::cdp::{CdpEvent, CdpRouter, lock};

/// Where the page scripts' messages go.
#[async_trait]
pub trait Inbox: Send + Sync {
    /// A request of plugin `plugin`'s page script in `view`: the answer for the page, or the
    /// code of why there is none.
    async fn request(&self, plugin: &str, view: ViewId, message: Value) -> Result<Value, String>;

    /// An event of plugin `plugin`'s page script in `view`, for its panels.
    fn emit(&self, plugin: &str, view: ViewId, message: Value);

    /// A line plugin `plugin`'s page script in `view` logged (RB-080-66): for rBrowser's log and
    /// console, never the page's.
    fn log(&self, plugin: &str, view: ViewId, line: PageLog);
}

/// A view's page session and the script each plugin has in it.
#[derive(Clone, Debug)]
struct Attached {
    view: ViewId,
    /// Plugin id to the `Page.addScriptToEvaluateOnNewDocument` identifier.
    scripts: HashMap<String, String>,
}

#[derive(Default)]
struct State {
    plugins: Vec<Arc<PageBridge>>,
    sessions: HashMap<String, Attached>,
    /// The contexts of the plugins' worlds: (session, context id) to plugin id.
    contexts: HashMap<(String, i64), String>,
}

impl State {
    fn plugin(&self, id: &str) -> Option<Arc<PageBridge>> {
        self.plugins
            .iter()
            .find(|bridge| bridge.plugin_id == id)
            .cloned()
    }

    fn drop_session(&mut self, session: &str) {
        self.sessions.remove(session);
        self.contexts.retain(|(owner, _), _| owner != session);
    }

    /// The contexts of `plugin`'s world in the sessions of `view` (every view for `None`).
    fn targets(&self, plugin: &str, view: Option<ViewId>) -> Vec<(String, i64)> {
        self.contexts
            .iter()
            .filter(|((session, _), owner)| {
                owner.as_str() == plugin
                    && self
                        .sessions
                        .get(session)
                        .is_some_and(|attached| view.is_none_or(|view| attached.view == view))
            })
            .map(|((session, context), _)| (session.clone(), *context))
            .collect()
    }
}

/// The page scripts of every enabled plugin in every view.
pub struct PluginPages {
    me: Weak<PluginPages>,
    cdp: Arc<CdpRouter>,
    runtime: Handle,
    inbox: Weak<dyn Inbox>,
    state: Mutex<State>,
}

impl PluginPages {
    /// The page scripts over `cdp`, their messages to `inbox`; follows the browser's events on
    /// `runtime`.
    pub fn new(cdp: Arc<CdpRouter>, runtime: Handle, inbox: Weak<dyn Inbox>) -> Arc<Self> {
        let pages = Arc::new_cyclic(|me| Self {
            me: me.clone(),
            cdp,
            runtime,
            inbox,
            state: Mutex::new(State::default()),
        });
        pages.watch();
        pages
    }

    /// The plugins whose page scripts run from now on. A plugin left out has its script removed
    /// from new documents and its running scripts stopped (`rbPlugin.onStop`); one added is
    /// injected into every view at once, its current document included.
    pub fn set_plugins(&self, plugins: Vec<Arc<PageBridge>>) {
        let (removed, added, sessions) = {
            let mut state = lock(&self.state);
            let removed: Vec<Arc<PageBridge>> = state
                .plugins
                .iter()
                .filter(|old| !plugins.iter().any(|new| new.plugin_id == old.plugin_id))
                .cloned()
                .collect();
            let added: Vec<Arc<PageBridge>> = plugins
                .iter()
                .filter(|new| {
                    !state
                        .plugins
                        .iter()
                        .any(|old| old.plugin_id == new.plugin_id)
                })
                .cloned()
                .collect();
            state.plugins = plugins;
            let sessions: Vec<(String, Attached)> = state
                .sessions
                .iter()
                .map(|(session, attached)| (session.clone(), attached.clone()))
                .collect();
            (removed, added, sessions)
        };
        if removed.is_empty() && added.is_empty() {
            return;
        }
        let Some(pages) = self.me.upgrade() else {
            return;
        };
        self.runtime.spawn(async move {
            for bridge in &removed {
                pages.remove(bridge, &sessions).await;
            }
            for (session, attached) in sessions.iter().filter(|_| !added.is_empty()) {
                if let Err(error) = pages.enable(session).await {
                    tracing::debug!(%error, "plugin page scripts not injected");
                    continue;
                }
                for bridge in &added {
                    pages.inject(session, attached.view, bridge).await;
                }
            }
        });
    }

    /// The plugins whose page scripts run now.
    pub fn plugins(&self) -> Vec<Arc<PageBridge>> {
        lock(&self.state).plugins.clone()
    }

    /// The page session of `view` is `session` (a first attach or a reconnect): every enabled
    /// plugin's page script goes in. Older sessions of the view are forgotten; a session known
    /// already is left as it is.
    pub async fn attach(&self, view: ViewId, session: &str) {
        let plugins = {
            let mut state = lock(&self.state);
            if state.sessions.contains_key(session) {
                return;
            }
            let older: Vec<String> = state
                .sessions
                .iter()
                .filter(|(other, attached)| attached.view == view && other.as_str() != session)
                .map(|(other, _)| other.clone())
                .collect();
            for older in older {
                state.drop_session(&older);
            }
            state.sessions.insert(
                session.to_owned(),
                Attached {
                    view,
                    scripts: HashMap::new(),
                },
            );
            state.plugins.clone()
        };
        if plugins.is_empty() {
            return;
        }
        if let Err(error) = self.enable(session).await {
            tracing::debug!(%view, %error, "plugin page scripts not injected");
            return;
        }
        for bridge in &plugins {
            self.inject(session, view, bridge).await;
        }
    }

    /// The session `view` has scripts in, if any.
    pub fn session_of(&self, view: ViewId) -> Option<String> {
        lock(&self.state)
            .sessions
            .iter()
            .find(|(_, attached)| attached.view == view)
            .map(|(session, _)| session.clone())
    }

    /// The view is gone.
    pub fn forget(&self, view: ViewId) {
        let mut state = lock(&self.state);
        let sessions: Vec<String> = state
            .sessions
            .iter()
            .filter(|(_, attached)| attached.view == view)
            .map(|(session, _)| session.clone())
            .collect();
        for session in sessions {
            state.drop_session(&session);
        }
    }

    /// Hands `message` to the `rbPlugin.on` listeners of `plugin`'s page scripts in `view`
    /// (every view for `None`).
    pub async fn post(&self, plugin: &str, view: Option<ViewId>, message: &Value) {
        let (receiver, targets) = {
            let state = lock(&self.state);
            let Some(bridge) = state.plugin(plugin) else {
                return;
            };
            (bridge.receiver.clone(), state.targets(plugin, view))
        };
        let expression = receiver_call(&receiver, &rb_plugin_host::post_message(message));
        for (session, context) in targets {
            self.evaluate(&session, context, &expression).await;
        }
    }

    async fn enable(&self, session: &str) -> Result<(), EngineError> {
        self.cdp
            .call("Page.enable", json!({}), Some(session))
            .await?;
        self.cdp
            .call("Runtime.enable", json!({}), Some(session))
            .await?;
        Ok(())
    }

    async fn inject(&self, session: &str, view: ViewId, bridge: &PageBridge) {
        let binding = json!({ "name": bridge.binding, "executionContextName": bridge.world });
        let script = json!({
            "source": bridge.source(view.0),
            "worldName": bridge.world,
            "runImmediately": true,
        });
        let result = async {
            self.cdp
                .call("Runtime.addBinding", binding, Some(session))
                .await?;
            self.cdp
                .call(
                    "Page.addScriptToEvaluateOnNewDocument",
                    script,
                    Some(session),
                )
                .await
        }
        .await;
        match result {
            Ok(answer) => {
                let identifier = answer["identifier"].as_str().unwrap_or_default().to_owned();
                if let Some(attached) = lock(&self.state).sessions.get_mut(session) {
                    attached
                        .scripts
                        .insert(bridge.plugin_id.clone(), identifier);
                }
            }
            Err(error) => tracing::debug!(
                %view,
                plugin_id = bridge.plugin_id.as_str(),
                %error,
                "a plugin's page script was not injected"
            ),
        }
    }

    async fn remove(&self, bridge: &PageBridge, sessions: &[(String, Attached)]) {
        let stop = receiver_call(&bridge.receiver, &stop_message());
        for (session, attached) in sessions {
            if let Some(identifier) = attached.scripts.get(&bridge.plugin_id) {
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
                    tracing::debug!(%error, "a plugin's page script was not removed");
                }
            }
            let _ = self
                .cdp
                .call(
                    "Runtime.removeBinding",
                    json!({ "name": bridge.binding }),
                    Some(session),
                )
                .await;
        }
        // The contexts stay known: switched on again, the script runs in the same world of the
        // same document, and Chromium announces no new context for it (review finding F8). A
        // binding call meanwhile finds no enabled plugin and is ignored; a destroyed or cleared
        // context leaves with its event.
        let targets = {
            let mut state = lock(&self.state);
            for attached in state.sessions.values_mut() {
                attached.scripts.remove(&bridge.plugin_id);
            }
            state.targets(&bridge.plugin_id, None)
        };
        for (session, context) in targets {
            self.evaluate(&session, context, &stop).await;
        }
    }

    async fn evaluate(&self, session: &str, context: i64, expression: &str) {
        let params = json!({ "expression": expression, "contextId": context });
        if let Err(error) = self
            .cdp
            .call("Runtime.evaluate", params, Some(session))
            .await
        {
            tracing::debug!(%error, "a plugin's page script was not reached");
        }
    }

    fn watch(&self) {
        let mut events = self.cdp.subscribe();
        let weak = self.me.clone();
        self.runtime.spawn(async move {
            loop {
                let event = match events.recv().await {
                    Ok(event) => event,
                    Err(RecvError::Lagged(missed)) => {
                        tracing::warn!(missed, "CDP events dropped (plugin page scripts)");
                        continue;
                    }
                    Err(RecvError::Closed) => return,
                };
                let Some(pages) = weak.upgrade() else {
                    return;
                };
                pages.on_event(&event);
            }
        });
    }

    fn on_event(&self, event: &CdpEvent) {
        let params = &event.params;
        let Some(session) = event.session_id.as_deref() else {
            if event.method == "Target.detachedFromTarget"
                && let Some(session) = params["sessionId"].as_str()
            {
                lock(&self.state).drop_session(session);
            }
            return;
        };
        match event.method.as_str() {
            "Runtime.bindingCalled" => self.on_binding(session, params),
            "Runtime.executionContextCreated" => {
                let context = &params["context"];
                let (Some(id), Some(name)) = (context["id"].as_i64(), context["name"].as_str())
                else {
                    return;
                };
                let mut state = lock(&self.state);
                let owner = state
                    .plugins
                    .iter()
                    .find(|bridge| bridge.world == name)
                    .map(|bridge| bridge.plugin_id.clone());
                if let Some(owner) = owner {
                    state.contexts.insert((session.to_owned(), id), owner);
                }
            }
            "Runtime.executionContextDestroyed" => {
                if let Some(id) = params["executionContextId"].as_i64() {
                    lock(&self.state).contexts.remove(&(session.to_owned(), id));
                }
            }
            "Runtime.executionContextsCleared" => lock(&self.state)
                .contexts
                .retain(|(owner, _), _| owner != session),
            _ => {}
        }
    }

    fn on_binding(&self, session: &str, params: &Value) {
        let Some(name) = params["name"].as_str() else {
            return;
        };
        let Some(context) = params["executionContextId"].as_i64() else {
            return;
        };
        let (view, bridge) = {
            let state = lock(&self.state);
            let Some(bridge) = state.plugins.iter().find(|bridge| bridge.binding == name) else {
                return;
            };
            let Some(view) = state.sessions.get(session).map(|attached| attached.view) else {
                return;
            };
            // Only the plugin's own world holds its binding; anything else is refused.
            if state.contexts.get(&(session.to_owned(), context)) != Some(&bridge.plugin_id) {
                tracing::debug!(%view, "a plugin binding call from outside its world");
                return;
            }
            (view, bridge.clone())
        };
        let payload = params["payload"].as_str().unwrap_or_default();
        let reply_id = match parse_page_payload(payload) {
            Ok(PageInput::Emit { message }) => {
                if let Some(inbox) = self.inbox.upgrade() {
                    inbox.emit(&bridge.plugin_id, view, message);
                }
                return;
            }
            Ok(PageInput::Request { id, message }) => {
                self.spawn_request(session, context, view, bridge, id, message);
                return;
            }
            Ok(PageInput::Log(line)) => {
                if let Some(inbox) = self.inbox.upgrade() {
                    inbox.log(&bridge.plugin_id, view, line);
                }
                return;
            }
            Err(PageRefusal::TooLarge { id }) => id.map(|id| (id, codes::TOO_LARGE)),
            Err(PageRefusal::Malformed { id }) => id.map(|id| (id, codes::INVALID)),
        };
        tracing::debug!(%view, plugin_id = bridge.plugin_id.as_str(), "a page message refused");
        if let (Some((id, code)), Some(pages)) = (reply_id, self.me.upgrade()) {
            let session = session.to_owned();
            self.runtime.spawn(async move {
                let reply = reply_message(id, Err(code));
                let expression = receiver_call(&bridge.receiver, &reply);
                pages.evaluate(&session, context, &expression).await;
            });
        }
    }

    fn spawn_request(
        &self,
        session: &str,
        context: i64,
        view: ViewId,
        bridge: Arc<PageBridge>,
        id: u32,
        message: Value,
    ) {
        let (Some(pages), Some(inbox)) = (self.me.upgrade(), self.inbox.upgrade()) else {
            return;
        };
        let session = session.to_owned();
        self.runtime.spawn(async move {
            let answer = inbox.request(&bridge.plugin_id, view, message).await;
            let reply = reply_message(id, answer.as_ref().map_err(String::as_str));
            let expression = receiver_call(&bridge.receiver, &reply);
            pages.evaluate(&session, context, &expression).await;
        });
    }
}

#[cfg(test)]
#[path = "pages_tests.rs"]
mod tests;
