//! [`FirefoxEngine`]: one Firefox for all Firefox views of a window. The first view starts it,
//! each view is a window of its own, the event pump turns BiDi events into view events and
//! agent reports, and a Firefox that quits or crashes ends every view (`Exited { crashed }`),
//! which the external windows show and restart. An engine that replaces another on the same
//! profile ([`FirefoxEngine::after`]) starts its Firefox only once the old one has quit, and an
//! [`EmulationFilter`] decides what of a view's emulation reaches Firefox (the workspace trust's
//! headers).

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Weak};

use async_trait::async_trait;
use rb_engine::{
    Emulation, Engine, EngineError, EngineHealth, EngineKind, ViewHandle, ViewId, ViewSpec,
};
use rb_sync::{AgentCommand, AgentConfig, AgentMessage, parse_message};
use tokio::runtime::Handle;
use tokio::sync::broadcast;
use tokio::sync::broadcast::error::RecvError;

use super::agent::{self, channel, preload_source, view_of};
use super::replay::FirefoxReplay;
use super::session::Browser;
use super::view::{FirefoxView, ViewState};
use super::{AgentReport, FirefoxConfig, engine_error};
use crate::browsing_context::{
    close as context_cmd_close, context_of, create_window_in, destroyed, navigated,
};
use crate::client::lock;
use crate::protocol::Event;
use crate::script::{add_preload_script, channel_message};
use crate::storage::DEFAULT_SESSION;

/// Agent reports a slow subscriber may fall behind.
const REPORT_BUFFER: usize = 1024;

/// What of a view's emulation reaches Firefox: the app's rules over it (an untrusted workspace
/// sends no project headers), applied to every emulation a view gets.
pub type EmulationFilter = Arc<dyn Fn(ViewId, &Emulation) -> Emulation + Send + Sync>;

/// The Firefox engine.
pub struct FirefoxEngine {
    me: Weak<FirefoxEngine>,
    config: FirefoxConfig,
    runtime: Handle,
    agent_config: Mutex<AgentConfig>,
    filter: Mutex<Option<EmulationFilter>>,
    /// The engine this one replaces on the same profile: quit before this one starts Firefox.
    before: Mutex<Option<Arc<FirefoxEngine>>>,
    /// The running browser; held across its start and its quit, so neither overlaps another.
    browser: tokio::sync::Mutex<Option<Arc<Browser>>>,
    running: AtomicBool,
    views: Mutex<HashMap<ViewId, Arc<ViewState>>>,
    reports: broadcast::Sender<AgentReport>,
    navigations: broadcast::Sender<(ViewId, String)>,
}

impl FirefoxEngine {
    /// An engine that starts Firefox with `config` on its first view; its tasks run on
    /// `runtime` (the app's, never the CEF UI thread's).
    pub fn new(config: FirefoxConfig, runtime: Handle) -> Arc<Self> {
        let agent_config = config.agent_config;
        Arc::new_cyclic(|me| Self {
            me: me.clone(),
            config,
            runtime,
            agent_config: Mutex::new(agent_config),
            filter: Mutex::new(None),
            before: Mutex::new(None),
            browser: tokio::sync::Mutex::new(None),
            running: AtomicBool::new(false),
            views: Mutex::new(HashMap::new()),
            reports: broadcast::channel(REPORT_BUFFER).0,
            navigations: broadcast::channel(REPORT_BUFFER).0,
        })
    }

    /// The replay of this engine's views, for `rb_sync::EngineRouter`.
    pub fn replay(&self) -> Arc<FirefoxReplay> {
        Arc::new(FirefoxReplay::new(self.me.clone()))
    }

    /// What the views' agents report (Firefox as a sync source).
    pub fn reports(&self) -> broadcast::Receiver<AgentReport> {
        self.reports.subscribe()
    }

    /// Every committed navigation of a view to a new URL: the grid's stand-in follows it.
    pub fn navigations(&self) -> broadcast::Receiver<(ViewId, String)> {
        self.navigations.subscribe()
    }

    /// The Firefox version of the running browser.
    pub async fn version(&self) -> Option<String> {
        self.browser
            .lock()
            .await
            .as_ref()
            .map(|browser| browser.version.clone())
    }

    /// This engine replaces `old` on the same profile: its Firefox starts only once `old`'s has
    /// quit (two Firefox on one profile refuse each other).
    pub fn after(&self, old: Arc<FirefoxEngine>) {
        *lock(&self.before) = Some(old);
    }

    /// The rules every emulation passes before it reaches Firefox.
    pub fn set_emulation_filter(&self, filter: EmulationFilter) {
        *lock(&self.filter) = Some(filter);
    }

    /// `emulation` of `view` as it reaches Firefox.
    pub(crate) fn filtered(&self, view: ViewId, emulation: &Emulation) -> Emulation {
        let filter = lock(&self.filter).clone();
        match filter {
            Some(filter) => filter(view, emulation),
            None => emulation.clone(),
        }
    }

    /// Lets go of every key and button the replay holds in `view` (the person takes it over).
    pub async fn release_input(&self, view: ViewId) {
        if let Some(state) = self.view_state(view) {
            state.release_input().await;
        }
    }

    /// The agents' configuration now.
    pub fn agent_config(&self) -> AgentConfig {
        *lock(&self.agent_config)
    }

    /// The agents' configuration from now on; every running agent gets it at once.
    pub fn set_agent_config(&self, config: AgentConfig) {
        *lock(&self.agent_config) = config;
        for view in self.live_views() {
            self.runtime
                .spawn(configure(view.clone(), view.context.clone(), config));
        }
    }

    /// The state of the running view `view`.
    pub fn view_state(&self, view: ViewId) -> Option<Arc<ViewState>> {
        lock(&self.views).get(&view).cloned()
    }

    /// Every running view.
    pub fn live_views(&self) -> Vec<Arc<ViewState>> {
        lock(&self.views).values().cloned().collect()
    }

    /// `state`, the view `view`, was closed: it is no longer routed to.
    pub(crate) fn forget(&self, view: ViewId, state: &Arc<ViewState>) {
        let mut views = lock(&self.views);
        if views
            .get(&view)
            .is_some_and(|known| Arc::ptr_eq(known, state))
        {
            views.remove(&view);
        }
    }

    /// The running browser, started when there is none.
    async fn browser(&self) -> Result<Arc<Browser>, EngineError> {
        let mut slot = self.browser.lock().await;
        if let Some(browser) = slot.as_ref().filter(|browser| !browser.client.is_closed()) {
            return Ok(browser.clone());
        }
        let before = lock(&self.before).take();
        if let Some(before) = before {
            before.shutdown().await;
        }
        let (browser, events) = Browser::start(self.config.launch.as_ref())
            .await
            .map_err(engine_error)?;
        let browser = Arc::new(browser);
        tracing::info!(version = %browser.version, "Firefox started");
        self.running.store(true, Ordering::Release);
        *slot = Some(browser.clone());
        self.runtime
            .spawn(pump(self.me.clone(), Arc::downgrade(&browser), events));
        Ok(browser)
    }

    /// The running browser, if one runs; never starts one (copying cookies, RB-040-05).
    pub(crate) async fn running_browser(&self) -> Option<Arc<Browser>> {
        self.browser
            .lock()
            .await
            .as_ref()
            .filter(|browser| !browser.client.is_closed())
            .cloned()
    }

    /// The browser `browser` ended (quit, crash): every view on a closed session ends - as
    /// closed by the person when Firefox quit cleanly (it quits with its last window), else as
    /// crashed (RB-080-38).
    async fn browser_ended(&self, browser: &Weak<Browser>) {
        let crashed = {
            let mut slot = self.browser.lock().await;
            let current = slot
                .as_ref()
                .is_some_and(|running| std::ptr::eq(Arc::as_ptr(running), browser.as_ptr()));
            if current {
                *slot = None;
                self.running.store(false, Ordering::Release);
            }
            let clean = match browser.upgrade() {
                Some(browser) => browser.ended_cleanly().await,
                None => false,
            };
            if current {
                tracing::warn!(clean, "Firefox ended");
            }
            !clean
        };
        let ended: Vec<Arc<ViewState>> = {
            let mut views = lock(&self.views);
            let gone: Vec<ViewId> = views
                .iter()
                .filter(|(_, view)| view.client.is_closed())
                .map(|(id, _)| *id)
                .collect();
            gone.iter().filter_map(|id| views.remove(id)).collect()
        };
        for view in ended {
            view.ended(crashed);
        }
    }

    fn on_event(&self, event: &Event) {
        if let Some((context, url)) = navigated(&event.method, &event.params) {
            if let Some(view) = self.view_by_context(&context) {
                view.navigated(&url);
            }
        } else if let Some(context) = destroyed(&event.method, &event.params) {
            if let Some(view) = self.view_by_context(&context) {
                self.forget(view.id, &view);
                view.ended(false);
            }
        } else if let Some(message) = channel_message(&event.method, &event.params) {
            let Some(view) = view_of(&message.channel).and_then(|id| self.view_state(id)) else {
                return;
            };
            match parse_message(&message.data) {
                Ok(parsed) => self.on_agent(&view, &message.context, parsed),
                Err(error) => tracing::debug!(view = %view.id, %error, "agent message refused"),
            }
        }
    }

    fn on_agent(&self, view: &Arc<ViewState>, context: &str, message: AgentMessage) {
        let top = match &message {
            AgentMessage::Ready(ready) => {
                // A new document booted with the configuration of the injection: it gets the
                // current one.
                let config = *lock(&self.agent_config);
                self.runtime
                    .spawn(configure(view.clone(), context.to_owned(), config));
                ready.top
            }
            AgentMessage::Event(event) => event.top,
            AgentMessage::Bus(bus) => bus.top,
            AgentMessage::Detected(detected) => detected.top,
            AgentMessage::Vitals(vitals) => vitals.top,
            // The probe's framework report (RB-080-24); the hooks reach Chromium views only.
            AgentMessage::Framework(framework) => framework.top,
            // A media failure (RB-099-29); the codec notice is for Chromium views.
            AgentMessage::Media(media) => media.top,
        };
        // No subscriber is fine.
        let _sent = self.reports.send(AgentReport {
            view: view.id,
            top,
            message,
        });
    }

    fn view_by_context(&self, context: &str) -> Option<Arc<ViewState>> {
        lock(&self.views)
            .values()
            .find(|view| view.context == context)
            .cloned()
    }

    async fn open(&self, spec: &ViewSpec, id: ViewId) -> Result<Arc<FirefoxView>, EngineError> {
        let browser = self.browser().await?;
        let client = browser.client.clone();
        // A named session's window opens in its own user context (RB-040-05).
        let session = spec
            .breakpoint
            .session
            .as_deref()
            .unwrap_or(DEFAULT_SESSION);
        let user_context = browser.user_context(session).await.map_err(engine_error)?;
        let window = create_window_in(user_context.as_deref());
        let context =
            context_of(&client.send(window).await.map_err(engine_error)?).map_err(engine_error)?;
        browser.close_initial().await;
        let process_id = browser.process_id();
        match self
            .set_up(spec, id, &context, client.clone(), process_id)
            .await
        {
            Ok(view) => Ok(view),
            Err(error) => {
                // The window is open already: it goes with the failed start, so no rebuild
                // leaves a blank Firefox window behind (RB-080-38).
                tracing::info!(view = %id, %error, "the Firefox view did not start; its window closes");
                if let Some(state) = self.view_state(id) {
                    self.forget(id, &state);
                    state.ended(false);
                }
                if let Err(close_error) = client.send(context_cmd_close(&context)).await {
                    tracing::debug!(view = %id, error = %close_error, "the window did not close");
                }
                Err(error)
            }
        }
    }

    /// The agent, the view's state, its emulation, first page and place in the window
    /// `context`, which is open already; `process_id` is the Firefox that owns it.
    async fn set_up(
        &self,
        spec: &ViewSpec,
        id: ViewId,
        context: &str,
        client: crate::client::BidiClient,
        process_id: Option<u32>,
    ) -> Result<Arc<FirefoxView>, EngineError> {
        let context = context.to_owned();
        let config = *lock(&self.agent_config);
        match preload_source(&self.config.agent_source, &config) {
            Some(source) => {
                let preload = add_preload_script(&source, &channel(id), &[&context]);
                if let Err(error) = client.send(preload).await {
                    tracing::warn!(view = %id, %error, "the sync agent could not be added");
                }
            }
            None => {
                tracing::warn!(view = %id, "no sync agent built: the Firefox view is not synced")
            }
        }
        let state = Arc::new(ViewState::new(
            id,
            context,
            client,
            self.navigations.clone(),
            process_id,
        ));
        if let Some(old) = lock(&self.views).insert(id, state.clone()) {
            old.ended(false);
        }
        // The scale into the cell, when the app knows it already (RB-080-40): the first
        // emulation takes the cell's height.
        *lock(&state.cell) = spec.cell;
        let view = Arc::new(FirefoxView {
            state,
            engine: self.me.clone(),
        });
        view.apply_emulation(&spec.emulation).await?;
        if !spec.url.is_empty() {
            view.first_load(&spec.url).await?;
        }
        if let Some(rect) = spec.window
            && let Err(error) = view.set_window(rect).await
        {
            tracing::debug!(view = %id, %error, "the window could not be placed");
        }
        Ok(view)
    }
}

#[async_trait]
impl Engine for FirefoxEngine {
    fn kind(&self) -> EngineKind {
        EngineKind::Firefox
    }

    fn external(&self) -> bool {
        true
    }

    fn health(&self) -> EngineHealth {
        if self.running.load(Ordering::Acquire) {
            EngineHealth::Running
        } else {
            EngineHealth::Stopped
        }
    }

    async fn create_view(&self, spec: &ViewSpec) -> Result<Arc<dyn ViewHandle>, EngineError> {
        let id = spec.view.ok_or_else(|| {
            EngineError::Invalid("a Firefox view needs the id of its cell".to_owned())
        })?;
        let view = self.open(spec, id).await?;
        Ok(view)
    }

    /// Quits Firefox. The browser's slot stays locked until the process is gone, so a second
    /// call - or a successor's start ([`FirefoxEngine::after`]) - returns only then.
    async fn shutdown(&self) {
        let mut slot = self.browser.lock().await;
        let browser = slot.take();
        let views: Vec<Arc<ViewState>> = lock(&self.views).drain().map(|(_, view)| view).collect();
        for view in views {
            view.ended(false);
        }
        self.running.store(false, Ordering::Release);
        if let Some(browser) = browser {
            browser.stop().await;
        }
        drop(slot);
    }
}

/// Follows `browser`'s events until its connection ends.
async fn pump(
    engine: Weak<FirefoxEngine>,
    browser: Weak<Browser>,
    mut events: broadcast::Receiver<Event>,
) {
    loop {
        let event = events.recv().await;
        let Some(owner) = engine.upgrade() else {
            return;
        };
        match event {
            Ok(event) => owner.on_event(&event),
            Err(RecvError::Lagged(missed)) => {
                tracing::warn!(missed, "Firefox events dropped");
            }
            Err(RecvError::Closed) => {
                owner.browser_ended(&browser).await;
                return;
            }
        }
    }
}

/// Sends `config` to the agent of `view` in `context`.
async fn configure(view: Arc<ViewState>, context: String, config: AgentConfig) {
    let command = AgentCommand::Configure { config };
    if let Err(error) = agent::command(&view.client, &context, &command).await {
        tracing::debug!(view = %view.id, %error, "the agent could not be configured");
    }
}

#[cfg(test)]
#[path = "engine_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "engine_cell_tests.rs"]
mod cell_tests;
