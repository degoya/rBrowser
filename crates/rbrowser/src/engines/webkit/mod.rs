//! WebKit through Playwright (RB-040-04): every view whose breakpoint names WebKit runs in a
//! window of its own, driven by Playwright in the Node sidecar - Playwright's WebKit build,
//! labelled "WebKit (not Safari)" everywhere, never Safari.
//!
//! | Part | Where |
//! | --- | --- |
//! | The engine and its views (`rb_engine::Engine`, `ViewHandle`) | [`engine`], [`view`] |
//! | The sidecar's `webkit.*` methods, emulation to context options, errors | [`rpc`] |
//! | The sidecar as the engine sees it | [`port`] |
//! | The SyncHub's replay into WebKit views | [`replay`] |
//! | The window follows its cell's navigations | [`follow`] |
//! | The sidecar side | `sidecar/src/webkit/` (launch, views, input, agent, the pinned download) |
//!
//! What WebKit cannot emulate is `rb_engine::EmulationReport`'s WebKit column (the view's
//! "not emulated" marker): `platform`, `prefers-reduced-data`, `prefers-reduced-transparency`,
//! `color-gamut`, network throttling, safe areas and the text scale. Without the WebKit build the
//! engine is unavailable and says to run `engines.update` (`Leader V U`); `engines.update`
//! installing it lets the views start again. Settings: `engines.webkit.enabled` (also the
//! command `engine.webkit.toggle`, `Leader V K`) and `engines.webkit.syncSource`.

pub mod engine;
mod follow;
pub mod port;
pub mod replay;
pub mod rpc;
pub mod view;

#[cfg(test)]
#[path = "e2e_tests.rs"]
mod e2e;
#[cfg(test)]
pub(crate) mod fake;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Weak};

use rb_commands::CommandError;
use rb_config::EngineKind;
use rb_engine::external::ExternalStatus;
use rb_engine::unsupported::UnsupportedEngine;
use rb_sync::AgentMessage;
use serde_json::{Value, json};

use super::HostEngines;
use crate::cdp::lock;
use crate::control::Core;
use crate::host::Host;
use crate::sync_agent::AGENT_SOURCE;

pub use engine::{ReportSink, WebkitEngine};
pub use port::{AppSidecar, SidecarPort};
pub use replay::WebkitReplay;

/// `Leader V K`: WebKit on or off for this session (`engines.webkit.enabled` is the default).
pub const TOGGLE: &str = "engine.webkit.toggle";

/// The WebKit engine of one window.
struct HostWebkit {
    host: Weak<Host>,
    engine: Arc<WebkitEngine>,
    /// The last `engines.webkit.enabled` applied: the toggle holds until the setting changes.
    setting: AtomicBool,
}

fn registry() -> &'static Mutex<Vec<HostWebkit>> {
    static REGISTRY: Mutex<Vec<HostWebkit>> = Mutex::new(Vec::new());
    &REGISTRY
}

/// The WebKit engine of `host`'s window, once it started.
pub fn for_host(host: &Host) -> Option<Arc<WebkitEngine>> {
    let mut entries = lock(registry());
    entries.retain(|entry| entry.host.strong_count() > 0);
    entries
        .iter()
        .find(|entry| std::ptr::eq(entry.host.as_ptr(), host))
        .map(|entry| entry.engine.clone())
}

/// The app's sidecar as WebKit's port; `None` without a launcher.
pub fn app_port() -> Option<Arc<dyn SidecarPort>> {
    crate::sidecar::shared().map(|sidecar| Arc::new(AppSidecar(sidecar)) as _)
}

/// WebKit for one window, built before its external windows start (`super::start_with`).
pub(crate) enum Prepared {
    /// Over a sidecar.
    Ready(Arc<WebkitEngine>),
    /// Without a sidecar: unavailable, and says why.
    Unavailable(Arc<UnsupportedEngine>),
}

/// The WebKit engine of `core`'s window over `port`, with the user's settings; starts nothing.
/// Runs outside the runtime (CEF's UI thread): its tasks go to `core.runtime()`.
pub(crate) fn prepare(core: &Core, port: Option<Arc<dyn SidecarPort>>) -> Prepared {
    let Some(port) = port else {
        let reason = rb_i18n::t("engines.webkit.noSidecar");
        let unavailable = UnsupportedEngine::because(EngineKind::Webkit, reason);
        return Prepared::Unavailable(Arc::new(unavailable));
    };
    let engine = WebkitEngine::new(port, core.runtime().clone(), AGENT_SOURCE);
    let user = core.user_settings();
    engine.set_enabled(user.engines.webkit.enabled);
    engine.set_sync(&sync_of(core), user.engines.webkit.sync_source);
    engine.set_sink(Some(sink_for(core.host())));
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

    /// Plugs in after the external windows started: the sync's router, the follow task, the
    /// toggle.
    pub(crate) fn attach(self, core: &Core) {
        let Self::Ready(engine) = self else {
            return;
        };
        if let Some(sync) = crate::sync_hub::for_host(core.host()) {
            let replay = Arc::new(WebkitReplay::new(&engine));
            sync.router().set_engine(EngineKind::Webkit, replay);
        }
        follow::watch(core.host(), &engine, core.runtime());
        lock(registry()).push(HostWebkit {
            host: Arc::downgrade(core.host()),
            engine,
            setting: AtomicBool::new(core.user_settings().engines.webkit.enabled),
        });
    }
}

/// The settings or the project changed: the sync source follows, and `engines.webkit.enabled`
/// when it changed (a toggle holds until then).
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
        entry
            .engine
            .set_sync(&sync_of(core), user.engines.webkit.sync_source);
        let wanted = user.engines.webkit.enabled;
        (entry.setting.swap(wanted, Ordering::AcqRel) != wanted).then(|| entry.engine.clone())
    };
    if let Some(engine) = changed {
        switch(core, &engine, user.engines.webkit.enabled);
    }
}

/// The project's `sync.*` with the scroll mode in effect - the one `sync_set` set at runtime,
/// else `sync.scrollMode` (RB-090-33): what WebKit's agents are configured from.
fn sync_of(core: &Core) -> rb_config::SyncConfig {
    let mut sync = core.host().config().sync;
    sync.scroll_mode = crate::scroll_sync::mode(core.host());
    sync
}

/// The scroll mode changed at runtime (`sync_set { scrollMode }`): the agents of the open WebKit
/// views follow it.
pub fn on_scroll_mode_changed(core: &Core) {
    if let Some(engine) = for_host(core.host()) {
        let source = core.user_settings().engines.webkit.sync_source;
        engine.set_sync(&sync_of(core), source);
    }
}

/// Runs `engine.webkit.toggle`; `None` for other commands.
pub fn run_command(core: &Core, id: &str, _args: &Value) -> Option<Result<Value, CommandError>> {
    if id != TOGGLE {
        return None;
    }
    let Some(engine) = for_host(core.host()) else {
        return Some(Err(CommandError::Failed(rb_i18n::t(
            "engines.webkit.noSidecar",
        ))));
    };
    let on = !engine.is_enabled();
    switch(core, &engine, on);
    Some(Ok(json!({ "enabled": on })))
}

/// WebKit on or off: off closes its windows (their cells show "switched off"), on starts them.
fn switch(core: &Core, engine: &Arc<WebkitEngine>, on: bool) {
    engine.set_enabled(on);
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

/// `engines.update` installed WebKit: every window's WebKit views start again.
pub fn installed() {
    let entries: Vec<(Weak<Host>, Arc<WebkitEngine>)> = lock(registry())
        .iter()
        .map(|entry| (entry.host.clone(), entry.engine.clone()))
        .collect();
    for (host, engine) in entries {
        engine.installed();
        let Some(host) = host.upgrade() else {
            continue;
        };
        if let Some(engines) = super::for_host(&host) {
            start_idle(&engines);
        }
    }
}

/// Starts every WebKit view that is neither running nor on its way: after the engine was
/// switched on or installed (a window the person closed included: they asked for WebKit).
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
        if view.engine == EngineKind::Webkit
            && idle
            && let Err(error) = windows.restart(EngineKind::Webkit, Some(view.view))
        {
            tracing::debug!(view = %view.view, %error, "the WebKit view did not start");
        }
    }
}

/// The SyncHub of `host`'s window takes the WebKit agents' events (`engines.webkit.syncSource`).
fn sink_for(host: &Arc<Host>) -> ReportSink {
    let host = Arc::downgrade(host);
    Arc::new(move |view, message| {
        let AgentMessage::Event(event) = message else {
            return;
        };
        let Some(host) = host.upgrade() else {
            return;
        };
        if let Some(sync) = crate::sync_hub::for_host(&host) {
            let route = sync.hub().on_event(view, &event);
            tracing::trace!(%view, seq = event.seq, ?route, "WebKit sync event");
        }
    })
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
