//! [`SyncAgents`]: the agents of every view, their CDP sessions (the page's and its iframes'),
//! their contexts and their reports.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, Weak};

use rb_engine::{EngineError, ViewId};
use rb_sync::{AgentCommand, AgentConfig, AgentMessage, BINDING_NAME, CommandResult, WORLD_NAME};
use serde_json::{Value, json};
use tokio::runtime::Handle;
use tokio::sync::broadcast;
use tokio::sync::broadcast::error::RecvError;

use super::contexts::ContextTable;
use super::{AGENT_SOURCE, boot_source, command_expression, inject};
use crate::cdp::{CdpEvent, CdpRouter, lock};

/// Reports kept for a subscriber that falls behind.
const REPORT_BUFFER: usize = 1024;
/// How often a new document's agent is configured again when the configuration changed while
/// it was being configured.
const READY_ROUNDS: usize = 4;

/// A message of a view's agent.
#[derive(Clone, Debug, PartialEq)]
pub struct AgentReport {
    /// The view.
    pub view: ViewId,
    /// The CDP frame of the agent, when its context is known.
    pub frame_id: Option<String>,
    /// What it said.
    pub message: AgentMessage,
}

/// A CDP session with agents in it.
#[derive(Clone, Debug)]
struct Session {
    view: ViewId,
    /// The page's session (not an iframe's): its target id is the top frame's id.
    top_frame: Option<String>,
}

#[derive(Default)]
struct State {
    sessions: HashMap<String, Session>,
    contexts: ContextTable,
    /// The configuration each view's agents should have (and get when they say ready).
    configs: HashMap<ViewId, AgentConfig>,
    /// Per view, counts every configuration set: a send of an older one is not the last word.
    generations: HashMap<ViewId, u64>,
    /// The views whose last configuration did not reach every agent: the next reconfigure
    /// sends it again.
    unconfirmed: HashSet<ViewId>,
}

impl State {
    /// `view` should have `config` from now on; its generation.
    fn set_config(&mut self, view: ViewId, config: AgentConfig) -> u64 {
        self.configs.insert(view, config);
        let generation = self.generations.entry(view).or_default();
        *generation += 1;
        *generation
    }

    fn generation(&self, view: ViewId) -> u64 {
        self.generations.get(&view).copied().unwrap_or_default()
    }

    /// Forgets every session of `view` - the page's and its iframes' - with their contexts.
    fn drop_sessions_of(&mut self, view: ViewId) {
        let sessions: Vec<String> = self
            .sessions
            .iter()
            .filter(|(_, entry)| entry.view == view)
            .map(|(session, _)| session.clone())
            .collect();
        for session in sessions {
            self.sessions.remove(&session);
            self.contexts.clear(&session);
        }
    }
}

/// The agents of every view.
pub struct SyncAgents {
    me: Weak<SyncAgents>,
    cdp: Arc<CdpRouter>,
    runtime: Handle,
    bundle: &'static str,
    state: Mutex<State>,
    reports: broadcast::Sender<AgentReport>,
}

impl SyncAgents {
    /// The agents over `cdp`, with the embedded bundle; follows the browser's events on
    /// `runtime`.
    pub fn new(cdp: Arc<CdpRouter>, runtime: Handle) -> Arc<Self> {
        Self::with_bundle(cdp, runtime, AGENT_SOURCE)
    }

    pub(crate) fn with_bundle(
        cdp: Arc<CdpRouter>,
        runtime: Handle,
        bundle: &'static str,
    ) -> Arc<Self> {
        let (reports, _) = broadcast::channel(REPORT_BUFFER);
        let agents = Arc::new_cyclic(|me| Self {
            me: me.clone(),
            cdp,
            runtime,
            bundle,
            state: Mutex::new(State::default()),
            reports,
        });
        agents.watch();
        agents
    }

    /// Every report from now on (the SyncHub, RB-020-02; the recorder, RB-050-01).
    pub fn subscribe(&self) -> broadcast::Receiver<AgentReport> {
        self.reports.subscribe()
    }

    /// Injects the agent into `view`'s page (`session`, attached to `target_id`) with `config`.
    /// Called after every attach; a view without a built bundle gets no agent.
    pub async fn attach(
        &self,
        view: ViewId,
        target_id: &str,
        session: &str,
        config: AgentConfig,
    ) -> Result<(), EngineError> {
        {
            let mut state = lock(&self.state);
            // A re-attach (renderer crash, reconnect): the view's older sessions and their
            // contexts are dead, and their agents must neither answer nor report.
            state.drop_sessions_of(view);
            state.set_config(view, config);
            state.unconfirmed.remove(&view);
            state.sessions.insert(
                session.to_owned(),
                Session {
                    view,
                    top_frame: Some(target_id.to_owned()),
                },
            );
        }
        self.inject_into(session, config).await
    }

    async fn inject_into(&self, session: &str, config: AgentConfig) -> Result<(), EngineError> {
        let Some(source) = boot_source(self.bundle, &config) else {
            tracing::warn!("the sync agent is not built (agent/: npm run build); no sync");
            return Ok(());
        };
        inject(&self.cdp, session, &source).await
    }

    /// The configuration `view`'s agents have (or get when they say ready).
    pub fn config(&self, view: ViewId) -> Option<AgentConfig> {
        lock(&self.state).configs.get(&view).copied()
    }

    /// Whether `view` has agents whose configuration is not `wanted` - another one, or one that
    /// did not reach every agent.
    pub fn needs_config(&self, view: ViewId, wanted: &AgentConfig) -> bool {
        let state = lock(&self.state);
        state
            .configs
            .get(&view)
            .is_some_and(|has| has != wanted || state.unconfirmed.contains(&view))
    }

    /// Sets `view`'s configuration (active view, `sync.*`) in every agent of the view. When it
    /// does not reach one, the view counts as unconfigured until a later configure does.
    pub async fn configure(&self, view: ViewId, config: AgentConfig) {
        let (generation, targets): (u64, Vec<(String, i64)>) = {
            let mut state = lock(&self.state);
            let generation = state.set_config(view, config);
            let sessions: Vec<String> = state
                .sessions
                .iter()
                .filter(|(_, entry)| entry.view == view)
                .map(|(session, _)| session.clone())
                .collect();
            let targets = sessions
                .into_iter()
                .flat_map(|session| {
                    let contexts = state.contexts.contexts(&session);
                    contexts.into_iter().map(move |id| (session.clone(), id))
                })
                .collect();
            (generation, targets)
        };
        let command = AgentCommand::Configure { config };
        let mut reached = true;
        for (session, context) in targets {
            if let Err(error) = self.evaluate(&session, context, &command).await {
                tracing::debug!(%view, %error, "sync agent not configured");
                reached = false;
            }
        }
        let mut state = lock(&self.state);
        // A newer configuration has its own say.
        if state.generation(view) == generation {
            if reached {
                state.unconfirmed.remove(&view);
            } else {
                state.unconfirmed.insert(view);
            }
        }
    }

    /// Configures the agent of a new document (`context` in `session`) with the view's
    /// configuration at the time of the send, again while it changes meanwhile.
    async fn configure_ready(&self, view: ViewId, session: &str, context: i64) {
        for _ in 0..READY_ROUNDS {
            let Some((generation, config)) = self.current(view) else {
                return;
            };
            let command = AgentCommand::Configure { config };
            if let Err(error) = self.evaluate(session, context, &command).await {
                tracing::debug!(%view, %error, "sync agent not configured");
                let mut state = lock(&self.state);
                if state.generation(view) == generation {
                    state.unconfirmed.insert(view);
                }
                return;
            }
            if self.current(view).map(|(now, _)| now) == Some(generation) {
                return;
            }
        }
    }

    /// `view`'s configuration and its generation.
    fn current(&self, view: ViewId) -> Option<(u64, AgentConfig)> {
        let state = lock(&self.state);
        let config = state.configs.get(&view).copied()?;
        Some((state.generation(view), config))
    }

    /// Runs `command` in the agent of `view`'s top frame.
    pub async fn command(
        &self,
        view: ViewId,
        command: &AgentCommand,
    ) -> Result<CommandResult, EngineError> {
        let (session, context) = self.top_context(view)?;
        self.evaluate(&session, context, command).await
    }

    /// Evaluates `expression` in the agent's world of `view`'s top frame and answers its value
    /// (`returnByValue`): the input replay's prologue (RB-020-03) - the echo marker, the submit
    /// guard and the resolution in one round trip. The page's main world never sees it.
    pub async fn evaluate_in_world(
        &self,
        view: ViewId,
        expression: &str,
    ) -> Result<Value, EngineError> {
        let (session, context) = self.top_context(view)?;
        self.evaluate_value(&session, context, expression).await
    }

    /// The session and agent context of `view`'s top frame.
    fn top_context(&self, view: ViewId) -> Result<(String, i64), EngineError> {
        let state = lock(&self.state);
        state
            .sessions
            .iter()
            .find_map(|(session, entry)| {
                let frame = entry.top_frame.as_deref().filter(|_| entry.view == view)?;
                let context = state.contexts.context_of(session, frame)?;
                Some((session.clone(), context))
            })
            .ok_or_else(|| EngineError::Invalid(format!("no sync agent in view {view}")))
    }

    /// The views that have an agent session.
    #[cfg(test)]
    pub(crate) fn views(&self) -> Vec<ViewId> {
        let state = lock(&self.state);
        state.sessions.values().map(|entry| entry.view).collect()
    }

    /// The view is gone: its sessions and configuration are forgotten.
    pub fn forget(&self, view: ViewId) {
        let mut state = lock(&self.state);
        state.drop_sessions_of(view);
        state.configs.remove(&view);
        state.unconfirmed.remove(&view);
    }

    async fn evaluate(
        &self,
        session: &str,
        context: i64,
        command: &AgentCommand,
    ) -> Result<CommandResult, EngineError> {
        let value = self
            .evaluate_value(session, context, &command_expression(command)?)
            .await?;
        serde_json::from_value(value)
            .map_err(|error| EngineError::Invalid(format!("the sync agent's answer: {error}")))
    }

    async fn evaluate_value(
        &self,
        session: &str,
        context: i64,
        expression: &str,
    ) -> Result<Value, EngineError> {
        let reply = self
            .cdp
            .call(
                "Runtime.evaluate",
                json!({
                    "expression": expression,
                    "contextId": context,
                    "returnByValue": true,
                }),
                Some(session),
            )
            .await?;
        if reply.get("exceptionDetails").is_some() {
            return Err(EngineError::Invalid(
                "the sync agent's command threw".into(),
            ));
        }
        reply
            .pointer("/result/value")
            .cloned()
            .ok_or_else(|| EngineError::Invalid("the sync agent gave no answer".into()))
    }

    fn watch(&self) {
        let mut events = self.cdp.subscribe();
        let weak = self.me.clone();
        self.runtime.spawn(async move {
            loop {
                let event = match events.recv().await {
                    Ok(event) => event,
                    Err(RecvError::Lagged(missed)) => {
                        tracing::warn!(missed, "CDP events dropped (sync agents)");
                        continue;
                    }
                    Err(RecvError::Closed) => return,
                };
                let Some(agents) = weak.upgrade() else {
                    return;
                };
                agents.on_event(&event);
            }
        });
    }

    fn on_event(&self, event: &CdpEvent) {
        let Some(session) = event.session_id.as_deref() else {
            if event.method == "Target.detachedFromTarget" {
                self.on_detached(&event.params);
            }
            return;
        };
        let params = &event.params;
        match event.method.as_str() {
            "Runtime.bindingCalled" => self.on_binding(session, params),
            "Runtime.executionContextCreated" => {
                let context = &params["context"];
                let (Some(id), Some(frame)) = (
                    context["id"].as_i64(),
                    context.pointer("/auxData/frameId").and_then(Value::as_str),
                ) else {
                    return;
                };
                // Kept even before the session is known: an iframe's `runImmediately` world
                // can be reported before its `Target.attachedToTarget` is handled. Only the
                // agent's world is kept, and a detach prunes it.
                if context["name"] == WORLD_NAME {
                    lock(&self.state).contexts.insert(session, frame, id);
                }
            }
            "Runtime.executionContextDestroyed" => {
                if let Some(id) = params["executionContextId"].as_i64() {
                    lock(&self.state).contexts.remove(session, id);
                }
            }
            "Runtime.executionContextsCleared" => lock(&self.state).contexts.clear(session),
            "Target.attachedToTarget" => self.on_attached(session, params),
            "Target.detachedFromTarget" => self.on_detached(params),
            _ => {}
        }
    }

    /// An iframe of a known session was attached: it gets the view's agent too.
    fn on_attached(&self, parent: &str, params: &Value) {
        if params.pointer("/targetInfo/type").and_then(Value::as_str) != Some("iframe") {
            return;
        }
        let Some(child) = params["sessionId"].as_str() else {
            return;
        };
        let config = {
            let mut state = lock(&self.state);
            let Some(view) = state.sessions.get(parent).map(|entry| entry.view) else {
                return;
            };
            state.sessions.insert(
                child.to_owned(),
                Session {
                    view,
                    top_frame: None,
                },
            );
            state.configs.get(&view).copied()
        };
        let (Some(config), Some(agents)) = (config, self.me.upgrade()) else {
            return;
        };
        let child = child.to_owned();
        self.runtime.spawn(async move {
            if let Err(error) = agents.inject_into(&child, config).await {
                tracing::debug!(%error, "sync agent not injected into an iframe");
            }
        });
    }

    fn on_detached(&self, params: &Value) {
        if let Some(session) = params["sessionId"].as_str() {
            let mut state = lock(&self.state);
            state.sessions.remove(session);
            state.contexts.clear(session);
        }
    }

    fn on_binding(&self, session: &str, params: &Value) {
        if params["name"] != BINDING_NAME {
            return;
        }
        let context = params["executionContextId"].as_i64();
        let (view, frame_id) = {
            let state = lock(&self.state);
            let Some(view) = state.sessions.get(session).map(|entry| entry.view) else {
                return;
            };
            let frame = context
                .and_then(|id| state.contexts.frame_of(session, id))
                .map(str::to_owned);
            (view, frame)
        };
        let payload = params["payload"].as_str().unwrap_or_default();
        let message = match rb_sync::parse_message(payload) {
            Ok(message) => message,
            Err(error) => {
                tracing::debug!(%view, %error, "sync agent message refused");
                return;
            }
        };
        // A new document's agent booted with the configuration of the inject; it gets the
        // current one (the active view may have changed since) - read when it is sent, so a
        // configure meanwhile is not overwritten by an older one.
        if let (AgentMessage::Ready(_), Some(context), Some(agents)) =
            (&message, context, self.me.upgrade())
        {
            let session = session.to_owned();
            self.runtime.spawn(async move {
                agents.configure_ready(view, &session, context).await;
            });
        }
        // No subscriber is not an error.
        let _ = self.reports.send(AgentReport {
            view,
            frame_id,
            message,
        });
    }
}

#[cfg(test)]
#[path = "agents_tests.rs"]
mod tests;
