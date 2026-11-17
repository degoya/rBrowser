//! [`WebkitEngine`]: WebKit through Playwright in the Node sidecar (RB-040-04), one external
//! window - and since RB-080-40 one WebKit process - per view. The engine starts with its first
//! view: it subscribes to the sidecar's notifications, readies WebKit (`webkit.launch`, the sync
//! agent's bundle with it) and opens the view in a browser of its own, whose process id names
//! the view's window. One task per sidecar process hands the notifications to the views; when
//! the sidecar ends, every view ends as crashed (the badge "engine stopped"), and
//! `engine_restart` or the automatic restart brings them back on a new process.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use async_trait::async_trait;
use rb_config::{EngineKind, SyncConfig};
use rb_engine::{Engine, EngineError, EngineHealth, ViewHandle, ViewId, ViewSpec};
use rb_sidecar::SidecarError;
use rb_sync::{AgentCommand, AgentConfig, AgentMessage, parse_message};
use serde_json::{Value, json};
use tokio::runtime::Handle;

use super::port::{Notifications, SidecarPort};
use super::rpc::{self, AgentNote, EventNote, context_options_in, engine_error};
use super::view::WebkitView;
use crate::cdp::lock;
use crate::sync_agent::agent_config;

/// How long a new view waits for its first commit to be handed out.
const FIRST_COMMIT_WAIT: Duration = Duration::from_secs(1);

/// Where the reports of the WebKit views' agents go (the SyncHub), when the views are a source.
pub type ReportSink = Arc<dyn Fn(ViewId, AgentMessage) + Send + Sync>;

#[derive(Clone, Debug, PartialEq, Eq)]
enum Health {
    Stopped,
    Running,
    NotInstalled(String),
}

struct State {
    enabled: bool,
    sync_source: bool,
    health: Health,
    views: HashMap<ViewId, Arc<WebkitView>>,
    /// A task hands out the notifications of the current sidecar process.
    listening: bool,
    agent: AgentConfig,
    sink: Option<ReportSink>,
}

/// The WebKit engine.
pub struct WebkitEngine {
    me: Weak<WebkitEngine>,
    port: Arc<dyn SidecarPort>,
    runtime: Handle,
    /// The sync agent's bundle (`crate::sync_agent::AGENT_SOURCE`); empty: no agent.
    bundle: &'static str,
    state: Mutex<State>,
    /// One start at a time.
    starting: tokio::sync::Mutex<()>,
}

impl WebkitEngine {
    /// The engine over `port`; its tasks run on `runtime`. Starts nothing.
    pub fn new(port: Arc<dyn SidecarPort>, runtime: Handle, bundle: &'static str) -> Arc<Self> {
        Arc::new_cyclic(|me| Self {
            me: me.clone(),
            port,
            runtime,
            bundle,
            state: Mutex::new(State {
                enabled: true,
                sync_source: true,
                health: Health::Stopped,
                views: HashMap::new(),
                listening: false,
                agent: agent_config(&SyncConfig::default(), true),
                sink: None,
            }),
            starting: tokio::sync::Mutex::new(()),
        })
    }

    /// `engines.webkit.enabled`: off, the engine is unavailable and opens no view.
    pub fn set_enabled(&self, on: bool) {
        lock(&self.state).enabled = on;
    }

    /// Whether the engine is on.
    pub fn is_enabled(&self) -> bool {
        lock(&self.state).enabled
    }

    /// `engines.webkit.syncSource` and the project's `sync.*` (with the scroll mode in effect):
    /// whether the agents capture, and with what. Views opened from now on start with it; the
    /// open views' agents get it when it changed (RB-090-33).
    pub fn set_sync(&self, sync: &SyncConfig, source: bool) {
        let agent = agent_config(sync, source);
        let views: Vec<Arc<WebkitView>> = {
            let mut state = lock(&self.state);
            state.sync_source = source;
            if state.agent == agent {
                return;
            }
            state.agent = agent;
            state.views.values().cloned().collect()
        };
        if views.is_empty() {
            return;
        }
        self.runtime.spawn(async move {
            let configure = AgentCommand::Configure { config: agent };
            for view in views {
                if let Err(error) = view.agent(&configure).await {
                    tracing::debug!(view = %view.id(), %error, "a WebKit agent kept its configuration");
                }
            }
        });
    }

    /// Where the agents' reports go.
    pub fn set_sink(&self, sink: Option<ReportSink>) {
        lock(&self.state).sink = sink;
    }

    /// WebKit was installed (`engines.update`): the next view tries again.
    pub fn installed(&self) {
        let mut state = lock(&self.state);
        if matches!(state.health, Health::NotInstalled(_)) {
            state.health = Health::Stopped;
        }
    }

    /// The open view `view`.
    pub fn view(&self, view: ViewId) -> Option<Arc<WebkitView>> {
        lock(&self.state).views.get(&view).cloned()
    }

    /// Adds copied cookies (`cookies`, Playwright's shape) to `view`'s context and answers how
    /// many WebKit took and refused (RB-040-05). Never starts the sidecar: an unknown view or
    /// none running is [`EngineError::Closed`].
    pub async fn add_cookies(
        &self,
        view: ViewId,
        cookies: Value,
    ) -> Result<(usize, usize), EngineError> {
        if self.view(view).is_none() {
            return Err(EngineError::Closed);
        }
        let params = json!({ "view": view, "cookies": cookies });
        let answer = match self.port.request_if_running(rpc::COOKIES, params).await {
            Some(answer) => answer.map_err(|error| engine_error(rpc::COOKIES, error))?,
            None => return Err(EngineError::Closed),
        };
        let count = |key: &str| {
            answer
                .get(key)
                .and_then(Value::as_u64)
                .and_then(|count| usize::try_from(count).ok())
                .unwrap_or(0)
        };
        Ok((count("added"), count("refused")))
    }

    /// `view` was closed: it is no longer the engine's (a newer view of its id stays).
    pub(super) fn forget(&self, view: &WebkitView) {
        let mut state = lock(&self.state);
        let id = ViewHandle::id(view);
        if state
            .views
            .get(&id)
            .is_some_and(|current| std::ptr::eq(current.as_ref(), view))
        {
            state.views.remove(&id);
        }
    }

    /// Subscribes and launches WebKit unless it runs.
    async fn ensure_started(&self) -> Result<(), EngineError> {
        let _one = self.starting.lock().await;
        if !lock(&self.state).listening {
            let notifications = self
                .port
                .subscribe()
                .await
                .map_err(|error| engine_error(rpc::LAUNCH, error))?;
            lock(&self.state).listening = true;
            let me = self.me.clone();
            self.runtime.spawn(listen(me, notifications));
        }
        let agent = (!self.bundle.trim().is_empty()).then_some(self.bundle);
        match self
            .port
            .request(rpc::LAUNCH, json!({ "agent": agent }))
            .await
        {
            Ok(_) => {
                lock(&self.state).health = Health::Running;
                Ok(())
            }
            Err(error) => {
                let missing = matches!(&error, SidecarError::Remote { code, .. } if *code == rpc::NOT_INSTALLED);
                let error = engine_error(rpc::LAUNCH, error);
                if missing && let EngineError::Unavailable { reason, .. } = &error {
                    lock(&self.state).health = Health::NotInstalled(reason.clone());
                }
                Err(error)
            }
        }
    }

    fn unavailable(&self) -> Option<String> {
        let state = lock(&self.state);
        if !state.enabled {
            return Some(rb_i18n::t("engines.webkit.disabled"));
        }
        match &state.health {
            Health::NotInstalled(reason) => Some(reason.clone()),
            Health::Stopped | Health::Running => None,
        }
    }

    fn on_event(&self, note: EventNote) {
        if note.kind == "disconnected" {
            return self.process_ended();
        }
        let Some(view) = note.view.and_then(|view| self.view(view)) else {
            return;
        };
        match note.kind.as_str() {
            "navigated" => view.navigated(note.url.unwrap_or_default()),
            "load" => view.updated(),
            "closed" => {
                lock(&self.state)
                    .views
                    .remove(&ViewHandle::id(view.as_ref()));
                view.ended(note.crashed);
            }
            _ => {}
        }
    }

    fn on_report(&self, note: AgentNote) {
        let sink = {
            let state = lock(&self.state);
            if !state.sync_source || !state.views.contains_key(&note.view) {
                return;
            }
            state.sink.clone()
        };
        let Some(sink) = sink else {
            return;
        };
        match parse_message(&note.payload) {
            Ok(message) => sink(note.view, message),
            Err(error) => {
                tracing::debug!(view = %note.view, %error, "a WebKit agent report was refused")
            }
        }
    }

    /// The sidecar or WebKit ended: every view ends as crashed.
    fn process_ended(&self) {
        let views: Vec<Arc<WebkitView>> = {
            let mut state = lock(&self.state);
            if state.health == Health::Running {
                state.health = Health::Stopped;
            }
            state.views.drain().map(|(_, view)| view).collect()
        };
        for view in views {
            view.ended(true);
        }
    }
}

/// Hands one sidecar process' notifications to the engine until the process ends.
async fn listen(engine: Weak<WebkitEngine>, mut notifications: Box<dyn Notifications>) {
    while let Some(notification) = notifications.recv().await {
        let Some(engine) = engine.upgrade() else {
            return;
        };
        match notification.method.as_str() {
            rpc::EVENT => match serde_json::from_value::<EventNote>(notification.params) {
                Ok(note) => engine.on_event(note),
                Err(error) => tracing::debug!(%error, "a webkit.event was refused"),
            },
            rpc::AGENT_REPORT => match serde_json::from_value::<AgentNote>(notification.params) {
                Ok(note) => engine.on_report(note),
                Err(error) => tracing::debug!(%error, "a webkit.agent was refused"),
            },
            _ => {}
        }
    }
    if let Some(engine) = engine.upgrade() {
        lock(&engine.state).listening = false;
        engine.process_ended();
    }
}

#[async_trait]
impl Engine for WebkitEngine {
    fn kind(&self) -> EngineKind {
        EngineKind::Webkit
    }

    fn external(&self) -> bool {
        true
    }

    fn health(&self) -> EngineHealth {
        if let Some(reason) = self.unavailable() {
            return EngineHealth::Unavailable(reason);
        }
        match lock(&self.state).health {
            Health::Running => EngineHealth::Running,
            Health::Stopped | Health::NotInstalled(_) => EngineHealth::Stopped,
        }
    }

    async fn create_view(&self, spec: &ViewSpec) -> Result<Arc<dyn ViewHandle>, EngineError> {
        let id = spec.view.ok_or_else(|| {
            EngineError::Invalid("a WebKit view needs the id of its cell".to_owned())
        })?;
        if let Some(reason) = self.unavailable() {
            return Err(EngineError::Unavailable {
                engine: EngineKind::Webkit,
                reason,
            });
        }
        self.ensure_started().await?;
        let agent = lock(&self.state).agent;
        let url = (!spec.url.is_empty()).then(|| spec.url.clone());
        let params = json!({
            "view": id,
            "url": url,
            "options": context_options_in(&spec.emulation, spec.cell),
            "agentConfig": agent,
        });
        // Registered first: the first navigation's notification may come before the answer.
        let view = WebkitView::new(
            id,
            self.port.clone(),
            self.me.clone(),
            spec.emulation.clone(),
            spec.url.clone(),
            spec.cell,
        );
        let replaced = lock(&self.state).views.insert(id, view.clone());
        if let Some(old) = replaced {
            old.ended(false);
        }
        let mut first = ViewHandle::events(view.as_ref());
        match self.port.request(rpc::CREATE, params).await {
            // The view's own browser process: the owner of its window (RB-080-40).
            Ok(answer) => view.set_process_id(
                answer
                    .get("pid")
                    .and_then(Value::as_u64)
                    .and_then(|pid| u32::try_from(pid).ok()),
            ),
            Err(error) => {
                lock(&self.state).views.remove(&id);
                return Err(engine_error(rpc::CREATE, error));
            }
        }
        if url.is_some() {
            // The sidecar announces the first commit before it answers; once it is handed out,
            // the caller's events start after it.
            let _ = tokio::time::timeout(FIRST_COMMIT_WAIT, first.recv()).await;
        }
        Ok(view)
    }

    async fn shutdown(&self) {
        let views: Vec<Arc<WebkitView>> = {
            let mut state = lock(&self.state);
            if state.health == Health::Running {
                state.health = Health::Stopped;
            }
            state.views.drain().map(|(_, view)| view).collect()
        };
        for view in views {
            view.end().await;
        }
        if let Some(Err(error)) = self
            .port
            .request_if_running(rpc::SHUTDOWN, Value::Null)
            .await
        {
            tracing::debug!(%error, "webkit.shutdown failed");
        }
    }
}

#[cfg(test)]
#[path = "engine_tests.rs"]
mod tests;
