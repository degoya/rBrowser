//! [`SafariEngine`]: the real Safari on macOS behind `rb_engine::Engine` (RB-095-03). One
//! `safaridriver` per window of rBrowser, started with the first Safari view; one WebDriver
//! session - one Safari window - per view. Whether Safari runs several sessions at once is the
//! spike's question: the driver's answer decides, and a refused second session leaves that view
//! unavailable with the reason ("one Safari view at a time"). When the driver ends, every view
//! of it ends as crashed and the external windows' restart brings them back on a new driver.
//! Remote automation switched off is the view's failure with the command to run, not the
//! engine's: once the person ran it, a restart works without a new start of rBrowser.
//!
//! The same engine runs iOS views ([`SafariEngine::ios`], RB-095-04): each view's session asks
//! for one simulator or device ([`crate::ios::resolve`] over the Mac's inventory, a shut-down
//! simulator booted first), and the view sizes no window.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, Weak};

use async_trait::async_trait;
use rb_engine::{Engine, EngineError, EngineHealth, EngineKind, ViewHandle, ViewId, ViewSpec};
use rb_sync::AgentConfig;
use rb_util::lock;
use tokio::sync::oneshot;

use crate::agent::{Agent, boot_script, follower_config, random_name};
use crate::client::DriverClient;
use crate::driver::{DriverProcess, Edition, Launch, Launched, capabilities};
use crate::emulation::position_body;
use crate::error::engine_error_for;
use crate::ios::{self, IosConfig, IosTarget, IosTools};
use crate::platform;
use crate::view::SafariView;

/// What the engine runs: `engines.safari.*`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SafariConfig {
    /// `engines.safari.enabled`: off, the engine is unavailable and opens no view.
    pub enabled: bool,
    /// `engines.safari.technologyPreview`.
    pub edition: Edition,
}

impl Default for SafariConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            edition: Edition::Safari,
        }
    }
}

/// The driver the views run on.
struct Driver {
    client: DriverClient,
    generation: u64,
    /// The edition it was started for: another one configured, the next view starts its own.
    edition: Edition,
    stop: Mutex<Option<oneshot::Sender<()>>>,
}

struct State {
    config: SafariConfig,
    /// `engines.ios.*` (an iOS engine only).
    ios: IosConfig,
    agent: AgentConfig,
    driver: Option<Arc<Driver>>,
    views: HashMap<ViewId, Arc<SafariView>>,
}

/// The Safari engine.
pub struct SafariEngine {
    me: Weak<SafariEngine>,
    launch: Arc<dyn Launch>,
    /// The Mac's iOS tools: `Some` makes it the iOS engine (RB-095-04).
    ios_tools: Option<Arc<dyn IosTools>>,
    /// The sync agent's bundle; empty: no agent.
    bundle: &'static str,
    state: Mutex<State>,
    generation: AtomicU64,
    /// One driver start at a time.
    starting: tokio::sync::Mutex<()>,
}

impl SafariEngine {
    /// The engine; its driver comes from `launch`. Starts nothing.
    pub fn new(launch: Arc<dyn Launch>, bundle: &'static str) -> Arc<Self> {
        Self::build(launch, None, bundle)
    }

    /// The iOS engine (RB-095-04): its views run in Mobile Safari on the simulator or device
    /// `tools` find; the driver comes from `launch`. Starts nothing.
    pub fn ios(
        launch: Arc<dyn Launch>,
        tools: Arc<dyn IosTools>,
        bundle: &'static str,
    ) -> Arc<Self> {
        Self::build(launch, Some(tools), bundle)
    }

    fn build(
        launch: Arc<dyn Launch>,
        ios_tools: Option<Arc<dyn IosTools>>,
        bundle: &'static str,
    ) -> Arc<Self> {
        Arc::new_cyclic(|me| Self {
            me: me.clone(),
            launch,
            ios_tools,
            bundle,
            state: Mutex::new(State {
                config: SafariConfig::default(),
                ios: IosConfig::default(),
                agent: follower_config(),
                driver: None,
                views: HashMap::new(),
            }),
            generation: AtomicU64::new(0),
            starting: tokio::sync::Mutex::new(()),
        })
    }

    /// `engines.safari.*` from now on; another edition takes effect with the next view, which
    /// starts that edition's driver (the old driver's views end as crashed and restart on it).
    pub fn set_config(&self, config: SafariConfig) {
        lock(&self.state).config = config;
    }

    /// The configuration in effect.
    pub fn config(&self) -> SafariConfig {
        lock(&self.state).config
    }

    /// `engines.ios.*` from now on (the iOS engine's views that start next).
    pub fn set_ios_config(&self, config: IosConfig) {
        lock(&self.state).ios = config;
    }

    /// The Mac's iOS tools of the iOS engine; `None` for Safari's.
    pub fn ios_tools(&self) -> Option<Arc<dyn IosTools>> {
        self.ios_tools.clone()
    }

    /// Safari, or iOS.
    fn engine_kind(&self) -> EngineKind {
        if self.ios_tools.is_some() {
            EngineKind::Ios
        } else {
            EngineKind::Safari
        }
    }

    /// The agents' configuration for the views that open from now on (a follower's: the app
    /// sends the project's sync switches with `active: false`).
    pub fn set_agent_config(&self, config: AgentConfig) {
        lock(&self.state).agent = config;
    }

    /// The open view `view`.
    pub fn view(&self, view: ViewId) -> Option<Arc<SafariView>> {
        lock(&self.state).views.get(&view).cloned()
    }

    /// `view` was closed: it is no longer the engine's (a newer view of its id stays).
    pub(crate) fn forget(&self, view: &SafariView) {
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

    /// The driver of `generation` ended: every view of it ends as crashed.
    pub(crate) fn driver_ended(&self, generation: u64) {
        for view in self.take_views(generation) {
            view.ended(true);
        }
    }

    /// The driver of `generation` is no longer the engine's, and its views neither.
    fn take_views(&self, generation: u64) -> Vec<Arc<SafariView>> {
        let mut state = lock(&self.state);
        if state
            .driver
            .as_ref()
            .is_some_and(|driver| driver.generation == generation)
        {
            state.driver = None;
        }
        let ended: Vec<ViewId> = state
            .views
            .iter()
            .filter(|(_, view)| view.generation() == generation)
            .map(|(id, _)| *id)
            .collect();
        ended
            .into_iter()
            .filter_map(|id| state.views.remove(&id))
            .collect()
    }

    /// A driver of another edition than the configured one: its views end as crashed (their
    /// restart opens them on the new driver), their sessions closed, the driver stopped.
    async fn retire(&self, driver: &Driver) {
        for view in self.take_views(driver.generation) {
            if view.ended(true) {
                view.end_session().await;
            }
        }
        if let Some(stop) = lock(&driver.stop).take() {
            // The watcher is gone when the process ended already.
            let _sent = stop.send(());
        }
    }

    fn unavailable(&self) -> Option<String> {
        (!lock(&self.state).config.enabled).then(|| rb_i18n::t("engines.safari.disabled"))
    }

    /// The driver, started unless it runs.
    async fn driver(&self) -> Result<Arc<Driver>, EngineError> {
        let _one = self.starting.lock().await;
        let (running, edition) = {
            let state = lock(&self.state);
            (state.driver.clone(), state.config.edition)
        };
        if let Some(driver) = running {
            if driver.edition == edition {
                return Ok(driver);
            }
            tracing::info!(?edition, "the Safari edition changed: a new safaridriver");
            self.retire(&driver).await;
        }
        let kind = self.engine_kind();
        let Launched { client, process } = self
            .launch
            .launch(edition)
            .await
            .map_err(|error| engine_error_for(kind, error))?;
        let generation = self.generation.fetch_add(1, Ordering::AcqRel) + 1;
        let (stop, stopped) = oneshot::channel();
        let driver = Arc::new(Driver {
            client,
            generation,
            edition,
            stop: Mutex::new(Some(stop)),
        });
        lock(&self.state).driver = Some(driver.clone());
        if let Some(process) = process {
            tokio::spawn(watch(self.me.clone(), generation, process, stopped));
        }
        if kind == EngineKind::Safari && platform::cached_version().is_none() {
            tokio::spawn(async move {
                platform::detect_version(edition).await;
            });
        }
        Ok(driver)
    }

    /// The simulator or device a view of `spec` runs on (the iOS engine): resolved in the
    /// inventory taken now, a shut-down simulator booted.
    async fn ios_target(
        &self,
        tools: &dyn IosTools,
        spec: &ViewSpec,
    ) -> Result<IosTarget, EngineError> {
        let config = lock(&self.state).ios.clone();
        let inventory = ios::take_inventory(tools).await;
        let target = ios::resolve(&config, &inventory, spec.breakpoint.ios_device.as_deref())
            .map_err(|refused| EngineError::Unavailable {
                engine: EngineKind::Ios,
                reason: refused.reason(),
            })?;
        if let IosTarget::Simulator(simulator) = &target
            && !simulator.booted
        {
            tools
                .boot(simulator, config.boot_timeout)
                .await
                .map_err(|error| engine_error_for(EngineKind::Ios, error))?;
        }
        tracing::debug!(view = ?spec.view, target = %ios::udid::short(target.udid()), "iOS view");
        Ok(target)
    }

    /// The view's first steps: the viewport, the window's corner, the first page.
    async fn start(view: &SafariView, spec: &ViewSpec) -> Result<(), EngineError> {
        let _turn = view.turn().await?;
        if view.ios() {
            // A simulator's or a device's screen: nothing to size or place.
            return if spec.url.is_empty() {
                Ok(())
            } else {
                view.load(&spec.url).await
            };
        }
        if let Err(error) = view.fit(&spec.emulation).await {
            tracing::debug!(view = %ViewHandle::id(view), %error, "the Safari window kept its size");
        }
        if let Some(rect) = spec.window {
            let placed = view.session().set_window_rect(position_body(rect)).await;
            if let Err(error) = view.checked(placed) {
                tracing::debug!(view = %ViewHandle::id(view), %error, "the Safari window stayed where it opened");
            }
        }
        if spec.url.is_empty() {
            return Ok(());
        }
        view.load(&spec.url).await
    }
}

/// Owns one driver process: its end ends its views; a stop ends it.
async fn watch(
    engine: Weak<SafariEngine>,
    generation: u64,
    mut process: DriverProcess,
    stop: oneshot::Receiver<()>,
) {
    let stopped = tokio::select! {
        () = process.exited() => false,
        _ = stop => true,
    };
    if stopped {
        process.stop().await;
    } else if let Some(engine) = engine.upgrade() {
        tracing::info!("safaridriver ended");
        engine.driver_ended(generation);
    }
}

#[async_trait]
impl Engine for SafariEngine {
    fn kind(&self) -> EngineKind {
        self.engine_kind()
    }

    fn external(&self) -> bool {
        true
    }

    fn health(&self) -> EngineHealth {
        if let Some(reason) = self.unavailable() {
            return EngineHealth::Unavailable(reason);
        }
        if lock(&self.state).driver.is_some() {
            EngineHealth::Running
        } else {
            EngineHealth::Stopped
        }
    }

    async fn create_view(&self, spec: &ViewSpec) -> Result<Arc<dyn ViewHandle>, EngineError> {
        let kind = self.engine_kind();
        let id = spec.view.ok_or_else(|| {
            EngineError::Invalid("a Safari view needs the id of its cell".to_owned())
        })?;
        if let Some(reason) = self.unavailable() {
            return Err(EngineError::Unavailable {
                engine: kind,
                reason,
            });
        }
        let target = match &self.ios_tools {
            Some(tools) => Some(self.ios_target(tools.as_ref(), spec).await?),
            None => None,
        };
        let driver = self.driver().await?;
        let agent_config = lock(&self.state).agent;
        let asked = target
            .as_ref()
            .map_or_else(|| capabilities(driver.edition), ios::capabilities);
        let session = match driver.client.new_session(asked).await {
            Ok(session) => session,
            Err(error) => {
                if error.driver_gone() {
                    self.driver_ended(driver.generation);
                }
                return Err(engine_error_for(kind, error));
            }
        };
        // Mobile Safari's version is not the Mac's Safari of Settings > About.
        if kind == EngineKind::Safari
            && let Some(version) = session.browser_version()
        {
            platform::remember_version(version);
        }
        let name = random_name();
        let agent = Agent {
            boot: boot_script(self.bundle, &agent_config, &name).map(Arc::from),
            name,
        };
        let view = Arc::new(SafariView::new(
            id,
            (kind, target.as_ref().map(IosTarget::label)),
            session,
            agent,
            self.me.clone(),
            driver.generation,
        ));
        let replaced = lock(&self.state).views.insert(id, view.clone());
        if let Some(old) = replaced
            && old.ended(false)
        {
            old.end_session().await;
        }
        if let Err(error) = Self::start(&view, spec).await {
            self.forget(&view);
            if view.ended(false) {
                view.end_session().await;
            }
            return Err(error);
        }
        Ok(view)
    }

    async fn shutdown(&self) {
        let (views, driver) = {
            let mut state = lock(&self.state);
            let views: Vec<Arc<SafariView>> = state.views.drain().map(|(_, view)| view).collect();
            (views, state.driver.take())
        };
        for view in views {
            if view.ended(false) {
                view.end_session().await;
            }
        }
        if let Some(driver) = driver
            && let Some(stop) = lock(&driver.stop).take()
        {
            // The watcher is gone when the process ended already.
            let _sent = stop.send(());
        }
    }
}

#[cfg(test)]
#[path = "engine_tests.rs"]
mod tests;
