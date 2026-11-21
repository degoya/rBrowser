//! Safari on macOS (RB-095-03): every view whose breakpoint names Safari runs in a Safari window
//! of its own, driven over `safaridriver` (`rb_safari`). On Windows and Linux the engine is a
//! stand-in that says Safari runs only on macOS: the badge menu shows the row disabled with that
//! reason, `view.set_engine` refuses it, and the view stays with Chromium.
//!
//! | Part | Where |
//! | --- | --- |
//! | The driver, the engine, its views, the replay | `rb_safari::{driver, engine, view, replay}` |
//! | The window follows its cell's navigations | [`follow`] |
//! | `engines.safari.enabled`, `engines.safari.technologyPreview` | [`config_of`], [`on_config_changed`] |
//! | The Safari version of Settings > About | [`version`] |
//!
//! A Safari view only follows: Safari locks a person's input in a window under automation, so
//! its agent captures nothing and every view carries the marker `leader` in `views_list`
//! `unsupported` (`rb_engine::emulation_report::LEADER_FIELD`). The window is never embedded
//! in its cell: macOS lets no app own another app's window (RB-110-10 decides what can be done).

// iOS views follow their cells the same way (RB-095-04).
pub(in crate::engines) mod follow;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Weak};

use rb_config::{EngineKind, SafariSettings};
use rb_engine::external::ExternalStatus;
use rb_engine::unsupported::UnsupportedEngine;
use rb_safari::{DriverLaunch, Edition, Launch, SafariConfig, SafariEngine, SafariReplay};

use super::HostEngines;
use crate::cdp::lock;
use crate::control::Core;
use crate::host::Host;
use crate::sync_agent::{AGENT_SOURCE, agent_config};

/// The Safari engine of one window.
struct HostSafari {
    host: Weak<Host>,
    engine: Arc<SafariEngine>,
    /// The last `engines.safari.enabled` applied.
    setting: AtomicBool,
}

fn registry() -> &'static Mutex<Vec<HostSafari>> {
    static REGISTRY: Mutex<Vec<HostSafari>> = Mutex::new(Vec::new());
    &REGISTRY
}

/// The Safari engine of `host`'s window, once it started (a Mac only).
pub fn for_host(host: &Host) -> Option<Arc<SafariEngine>> {
    let mut entries = lock(registry());
    entries.retain(|entry| entry.host.strong_count() > 0);
    entries
        .iter()
        .find(|entry| std::ptr::eq(entry.host.as_ptr(), host))
        .map(|entry| entry.engine.clone())
}

/// This machine's `safaridriver`: on a Mac only.
pub fn app_launch() -> Option<Arc<dyn Launch>> {
    rb_safari::SUPPORTED.then(|| Arc::new(DriverLaunch) as Arc<dyn Launch>)
}

/// The engine's configuration of `engines.safari.*`.
pub fn config_of(settings: &SafariSettings) -> SafariConfig {
    SafariConfig {
        enabled: settings.enabled,
        edition: Edition::from_setting(settings.technology_preview),
    }
}

/// The Safari version for Settings > About and `app_info`: what a session or the bundle named;
/// `None` before either (and always outside macOS).
pub fn version() -> Option<String> {
    rb_safari::platform::cached_version()
}

/// Safari for one window, built before its external windows start (`super::start_engines`).
pub(crate) enum Prepared {
    /// Over `safaridriver`.
    Ready(Arc<SafariEngine>),
    /// No Safari here: unavailable, and says why.
    Unavailable(Arc<UnsupportedEngine>),
}

/// The Safari engine of `core`'s window over `launch` (`None`: not a Mac); starts nothing.
/// Runs outside the runtime (CEF's UI thread).
pub(crate) fn prepare(core: &Core, launch: Option<Arc<dyn Launch>>) -> Prepared {
    let Some(launch) = launch else {
        let reason = rb_i18n::t("engines.safari.onlyMacos");
        let unavailable = UnsupportedEngine::because(EngineKind::Safari, reason);
        return Prepared::Unavailable(Arc::new(unavailable));
    };
    let engine = SafariEngine::new(launch, AGENT_SOURCE);
    engine.set_config(config_of(&core.user_settings().engines.safari));
    engine.set_agent_config(agent_config(&sync_of(core), false));
    Prepared::Ready(engine)
}

impl Prepared {
    /// The engine the external windows get.
    pub(crate) fn engine(&self) -> Arc<dyn rb_engine::Engine> {
        match self {
            Self::Ready(engine) => engine.clone(),
            Self::Unavailable(stand_in) => stand_in.clone(),
        }
    }

    /// Plugs in after the external windows started: the sync's replay, the follow task.
    pub(crate) fn attach(self, core: &Core) {
        let Self::Ready(engine) = self else {
            return;
        };
        if let Some(sync) = crate::sync_hub::for_host(core.host()) {
            let replay = Arc::new(SafariReplay::new(&engine));
            sync.router().set_engine(EngineKind::Safari, replay);
        }
        follow::watch(core.host(), &engine, core.runtime());
        lock(registry()).push(HostSafari {
            host: Arc::downgrade(core.host()),
            engine,
            setting: AtomicBool::new(core.user_settings().engines.safari.enabled),
        });
    }
}

/// The settings or the project changed: the engine takes `engines.safari.*` (the edition with
/// its next driver) and the agents' sync switches; switched off its windows close, switched on
/// its views start again.
pub fn on_config_changed(core: &Core) {
    let user = core.user_settings();
    let changed = {
        let entries = lock(registry());
        let Some(entry) = entries
            .iter()
            .find(|entry| std::ptr::eq(entry.host.as_ptr(), core.host().as_ref()))
        else {
            return;
        };
        entry.engine.set_config(config_of(&user.engines.safari));
        entry
            .engine
            .set_agent_config(agent_config(&sync_of(core), false));
        let wanted = user.engines.safari.enabled;
        (entry.setting.swap(wanted, Ordering::AcqRel) != wanted).then(|| entry.engine.clone())
    };
    if let Some(engine) = changed {
        switch(core, &engine, user.engines.safari.enabled);
    }
}

/// The project's `sync.*` with the scroll mode in effect (RB-090-33); iOS's agents take it too.
pub(in crate::engines) fn sync_of(core: &Core) -> rb_config::SyncConfig {
    let mut sync = core.host().config().sync;
    sync.scroll_mode = crate::scroll_sync::mode(core.host());
    sync
}

/// Safari on or off: off closes its windows (their cells say "switched off"), on starts them.
fn switch(core: &Core, engine: &Arc<SafariEngine>, on: bool) {
    let Some(engines) = super::for_host(core.host()) else {
        return;
    };
    if on {
        start_idle(&engines);
    } else {
        let engine = engine.clone();
        core.runtime().spawn(async move {
            rb_engine::Engine::shutdown(engine.as_ref()).await;
        });
    }
}

/// Starts every Safari view that is neither running nor on its way.
fn start_idle(engines: &HostEngines) {
    let windows = engines.windows();
    for view in windows.list() {
        let idle = matches!(
            view.status,
            ExternalStatus::Unavailable
                | ExternalStatus::Failed
                | ExternalStatus::Exited
                | ExternalStatus::Closed
        );
        if view.engine == EngineKind::Safari
            && idle
            && let Err(error) = windows.restart(EngineKind::Safari, Some(view.view))
        {
            tracing::debug!(view = %view.view, %error, "the Safari view did not start");
        }
    }
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
