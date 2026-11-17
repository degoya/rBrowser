//! The engines in the app (RB-040-01): [`HostEngines`] runs `rb_engine::external::ExternalWindows`
//! for one window. Every view of the grid whose breakpoint names Firefox or WebKit is an
//! external view. Firefox runs over WebDriver BiDi ([`firefox`], RB-040-02), WebKit through
//! Playwright in the sidecar ([`webkit`], RB-040-04); an engine that cannot run shows why in its
//! cell's label strip (`crate::cell_info`, RB-090-27) while Chromium stands in for it. The
//! SyncHub reaches each view through `rb_sync::EngineRouter`.
//!
//! | Part | Where |
//! | --- | --- |
//! | Engine contract, Chromium behind it, fake engine, conformance | `rb_engine::{engine, chromium, test_support}` |
//! | External windows: placement, lifecycle, restart backoff | `rb_engine::external` |
//! | Routing the sync per engine | `rb_sync::EngineRouter` |
//! | Commands `view.external.*`, `engine.restart` | [`commands`] |
//! | Firefox: settings, engine, sync source, `engine.firefox.toggle` | [`firefox`] |
//! | MCP `engine_restart`, the engine fields of `views_list` | [`mcp`] |
//! | WebKit through Playwright in the sidecar (RB-040-04), `engine.webkit.toggle` | [`webkit`] |
//! | `view.set_engine` behind the engine badge of every label strip (RB-080-33) | [`set_engine`] |
//! | An external view in its grid cell: what is asked of the window, released, failed (RB-080-39) | [`embed`] |

pub mod commands;
pub mod embed;
pub mod firefox;
mod glue;
pub mod mcp;
pub mod set_engine;
pub mod webkit;

use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use rb_config::ExternalWindowSettings;
use rb_engine::external::Placement;

use crate::cdp::lock;
use crate::control::Core;
use crate::host::Host;

pub use glue::HostEngines;

/// `Leader V D`: place every external window again by `engines.external.placement`; with the
/// embedding on (RB-080-39) it first shows every embedded view as a window of its own.
pub const ARRANGE: &str = "view.external.arrange";
/// `Leader V W`: every external window to the front.
pub const RAISE: &str = "view.external.raise";
/// `Leader R E`: restart an engine's external views (`{ "engine"?: "firefox" | "webkit",
/// "view"?: id }`; the placeholder's "Restart" button).
pub const RESTART: &str = "engine.restart";

/// How long quitting waits for the external windows to close.
const SHUTDOWN_WAIT: Duration = Duration::from_secs(3);

/// The engines of every live window, by host.
fn registry() -> &'static Mutex<Vec<(Weak<Host>, Arc<HostEngines>)>> {
    static REGISTRY: Mutex<Vec<(Weak<Host>, Arc<HostEngines>)>> = Mutex::new(Vec::new());
    &REGISTRY
}

/// The engines of `host`'s window, once [`on_start`] ran for it.
pub fn for_host(host: &Host) -> Option<Arc<HostEngines>> {
    let mut entries = lock(registry());
    entries.retain(|(weak, _)| weak.strong_count() > 0);
    entries
        .iter()
        .find(|(weak, _)| std::ptr::eq(weak.as_ptr(), host))
        .map(|(_, engines)| engines.clone())
}

/// Runs the command `id` when it is one of the engines'; `None` otherwise.
pub fn run_command(
    core: &Core,
    id: &str,
    args: &serde_json::Value,
) -> Option<Result<serde_json::Value, rb_commands::CommandError>> {
    commands::run_command(core, id, args)
        .or_else(|| firefox::run_command(core, id, args))
        .or_else(|| set_engine::run_command(core, id, args))
}

/// Starts the external windows of `core`'s window, Firefox's and WebKit's engines among them;
/// WebKit runs over the app's sidecar.
pub fn on_start(core: &Core) {
    start_with(core, webkit::app_port());
}

/// [`on_start`] with WebKit over `webkit_port` (`None`: no sidecar, WebKit is unavailable). Both
/// engines are built before the first view starts, so no view starts on a stand-in.
pub(crate) fn start_with(core: &Core, webkit_port: Option<Arc<dyn webkit::SidecarPort>>) {
    if for_host(core.host()).is_some() {
        return;
    }
    let firefox = firefox::prepare(core);
    let webkit = webkit::prepare(core, webkit_port);
    let engines = HostEngines::start_with(
        core.host(),
        core.runtime(),
        &core.user_settings().engines.external,
        vec![firefox.engine(), webkit.engine()],
    );
    lock(registry()).push((Arc::downgrade(core.host()), engines.clone()));
    firefox.attach(&engines);
    webkit.attach(core);
}

/// The project or the user settings changed: the placement, the restart and the tiles follow,
/// and Firefox its settings.
pub fn on_config_changed(core: &Core) {
    if let Some(engines) = for_host(core.host()) {
        engines.apply_settings(&core.user_settings().engines.external);
    }
    firefox::on_config_changed(core);
    webkit::on_config_changed(core);
}

/// Closes every external window and stops the engines (the app quits); called outside the
/// runtime.
pub fn shutdown(runtime: &tokio::runtime::Runtime) {
    let all: Vec<Arc<HostEngines>> = lock(registry())
        .drain(..)
        .map(|(_, engines)| engines)
        .collect();
    for engines in all {
        let closing = engines.windows().clone();
        // The timeout is made inside the runtime: this runs on CEF's UI thread, outside it.
        let done = runtime
            .block_on(async move { tokio::time::timeout(SHUTDOWN_WAIT, closing.shutdown()).await });
        if done.is_err() {
            tracing::warn!("external windows did not close in time");
        }
    }
}

/// `engines.external.placement`, `beside` when it does not parse (the schema refuses such a
/// value; a hand-edited file may still carry one).
pub fn placement_of(settings: &ExternalWindowSettings) -> Placement {
    settings.placement.parse().unwrap_or_else(|error| {
        tracing::warn!(%error, "engines.external.placement");
        Placement::Beside
    })
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
