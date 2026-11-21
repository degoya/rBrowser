//! iOS on macOS (RB-095-04): every view whose breakpoint names `ios` runs in Mobile Safari - on
//! the iOS Simulator when Xcode is installed, else on an iPhone or iPad on USB - driven over
//! `safaridriver` (`rb_safari::ios`). The breakpoint's `iosDevice` names the model, simulator or
//! device; `engines.ios.*` the mode, the default model and the boot wait. On Windows and Linux
//! the engine is a stand-in that says iOS views run only on macOS.
//!
//! | Part | Where |
//! | --- | --- |
//! | The engine, its views, the replay, the inventory, the pick of the target | `rb_safari::{engine, ios, replay}` |
//! | `ios.devices` (`Leader B I`), MCP `ios_devices`, the badge menu's device rows | [`commands`] |
//! | The view follows its cell's navigations | `super::safari::follow` |
//! | `engines.ios.*` | [`config_of`], [`on_config_changed`] |
//!
//! An iOS view only follows, as a Safari view does (the marker `leader` in `views_list`
//! `unsupported`); the Simulator app's window and the device are never in the cell. A device's
//! UDID is cut short in every log line and masked in the diagnostics ZIP (`rb_diag::redact`).

pub mod commands;

use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use rb_config::{EngineKind, IosSettings};
use rb_engine::unsupported::UnsupportedEngine;
use rb_safari::ios::{HostTools, IosConfig, IosMode, IosTools};
use rb_safari::{DriverLaunch, Launch, SafariEngine, SafariReplay};

use crate::cdp::lock;
use crate::control::Core;
use crate::host::Host;
use crate::sync_agent::{AGENT_SOURCE, agent_config};

/// The longest `iosDevice` taken, in characters.
pub const MAX_DEVICE: usize = 80;
/// The boot wait's bounds, as the schema has them (`engines.ios.bootTimeoutSec`).
const BOOT_SECS: (u32, u32) = (10, 600);

/// The iOS engine of one window.
struct HostIos {
    host: Weak<Host>,
    engine: Arc<SafariEngine>,
}

fn registry() -> &'static Mutex<Vec<HostIos>> {
    static REGISTRY: Mutex<Vec<HostIos>> = Mutex::new(Vec::new());
    &REGISTRY
}

/// The iOS engine of `host`'s window, once it started (a Mac only).
pub fn for_host(host: &Host) -> Option<Arc<SafariEngine>> {
    let mut entries = lock(registry());
    entries.retain(|entry| entry.host.strong_count() > 0);
    entries
        .iter()
        .find(|entry| std::ptr::eq(entry.host.as_ptr(), host))
        .map(|entry| entry.engine.clone())
}

/// What the iOS engine runs on: the driver and the Mac's tools.
pub struct IosSetup {
    /// `safaridriver`.
    pub launch: Arc<dyn Launch>,
    /// `xcrun simctl`, `system_profiler`, `open`.
    pub tools: Arc<dyn IosTools>,
}

/// This Mac's driver and tools; `None` outside macOS.
pub fn app_setup() -> Option<IosSetup> {
    rb_safari::SUPPORTED.then(|| IosSetup {
        launch: Arc::new(DriverLaunch) as Arc<dyn Launch>,
        tools: Arc::new(HostTools) as Arc<dyn IosTools>,
    })
}

/// The engine's configuration of `engines.ios.*`; the boot wait held to the schema's range.
pub fn config_of(settings: &IosSettings) -> IosConfig {
    let seconds = settings.boot_timeout_sec.clamp(BOOT_SECS.0, BOOT_SECS.1);
    IosConfig {
        mode: IosMode::from_setting(&settings.mode),
        default_simulator: settings.default_simulator.trim().to_owned(),
        boot_timeout: Duration::from_secs(u64::from(seconds)),
    }
}

/// Whether `device` may be written as a breakpoint's `iosDevice`: 1 to [`MAX_DEVICE`]
/// characters of printable text.
pub fn valid_device(device: &str) -> bool {
    let trimmed = device.trim();
    !trimmed.is_empty()
        && device.chars().count() <= MAX_DEVICE
        && !device.chars().any(char::is_control)
}

/// iOS for one window, built before its external windows start (`super::start_engines_with`).
pub(crate) enum Prepared {
    /// Over `safaridriver`.
    Ready(Arc<SafariEngine>),
    /// No iOS here: unavailable, and says why.
    Unavailable(Arc<UnsupportedEngine>),
}

/// The iOS engine of `core`'s window over `setup` (`None`: not a Mac); starts nothing. Runs
/// outside the runtime (CEF's UI thread).
pub(crate) fn prepare(core: &Core, setup: Option<IosSetup>) -> Prepared {
    let Some(setup) = setup else {
        let reason = rb_i18n::t("engines.ios.onlyMacos");
        let unavailable = UnsupportedEngine::because(EngineKind::Ios, reason);
        return Prepared::Unavailable(Arc::new(unavailable));
    };
    let engine = SafariEngine::ios(setup.launch, setup.tools, AGENT_SOURCE);
    engine.set_ios_config(config_of(&core.user_settings().engines.ios));
    engine.set_agent_config(agent_config(&super::safari::sync_of(core), false));
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

    /// Plugs in after the external windows started: the sync's replay, the follow task, the
    /// first inventory for the badge menu (in the background).
    pub(crate) fn attach(self, core: &Core) {
        let Self::Ready(engine) = self else {
            return;
        };
        if let Some(sync) = crate::sync_hub::for_host(core.host()) {
            let replay = Arc::new(SafariReplay::new(&engine));
            sync.router().set_engine(EngineKind::Ios, replay);
        }
        super::safari::follow::watch(core.host(), &engine, core.runtime());
        if let Some(tools) = engine.ios_tools() {
            core.runtime().spawn(async move {
                rb_safari::ios::take_inventory(tools.as_ref()).await;
            });
        }
        lock(registry()).push(HostIos {
            host: Arc::downgrade(core.host()),
            engine,
        });
    }
}

/// The settings or the project changed: the engine takes `engines.ios.*` and the agents' sync
/// switches for the views that start next.
pub fn on_config_changed(core: &Core) {
    let Some(engine) = for_host(core.host()) else {
        return;
    };
    engine.set_ios_config(config_of(&core.user_settings().engines.ios));
    engine.set_agent_config(agent_config(&super::safari::sync_of(core), false));
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
