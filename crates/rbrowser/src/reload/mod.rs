//! Hot reload (RB-030-05): after a save every view is current. A watch of the project (the
//! system's file events, polling, or `rbrowser-wsl watch` inside WSL) hands debounced batches
//! to the [`HotReload`] service: stylesheets and images are swapped in place (JS state, scroll
//! position and form values stay), anything else reloads every view. "Reload CSS only"
//! (`reload.css_only`), the full reload (`reload.all`, MCP `reload`, `rbrowser --reload`) and
//! the session pause (`reload.toggle_pause`) are commands; the interval reload keeps a remote
//! page (staging) current.
//!
//! - [`classify`]: CSS, image or other; the plan of a batch.
//! - [`options`]: the `reload.*` settings in effect.
//! - [`source`]: the watch, native, polling or the WSL helper, with the polling fallback;
//!   [`watch_errors`] logs each of its errors once.
//! - [`perform`], [`script`]: the work in the views over CDP.
//! - [`interval`]: the interval reload's schedule.
//! - [`service`]: all of it for one window.
//! - [`mcp`], [`status`]: the tool `reload`, the watcher's state for the toolbar and
//!   `project_info`.

pub mod classify;
#[cfg(test)]
mod fake_host;
pub mod interval;
pub mod mcp;
pub mod options;
pub mod perform;
pub mod script;
pub mod service;
pub mod source;
pub mod status;
pub mod watch_errors;

use std::sync::{Arc, Mutex};

use rb_commands::CommandError;
use rb_engine::{ViewEvent, ViewHost as _};
use rb_wsl::settings::WslSettings;
use serde_json::{Value, json};
use tokio::runtime::Handle;
use tokio::sync::broadcast::error::RecvError;

pub use mcp::{ReloadAnswer, ReloadArgs, ReloadKind};
pub use options::ReloadOptions;
pub use service::HotReload;
pub use status::{WatcherInfo, WatcherStatus};

use crate::cdp::lock;
use crate::control::Core;

/// Reload every view (`⌘ R R`); MCP `reload`.
pub const RELOAD_ALL: &str = "reload.all";
/// Swap every stylesheet without a reload (`⌘ R C`); MCP `reload` with `mode: "css"`.
pub const RELOAD_CSS_ONLY: &str = "reload.css_only";
/// Pause or resume hot reload for this session (`⌘ R P`).
pub const TOGGLE_PAUSE: &str = "reload.toggle_pause";
/// Start the project's WSL helper again (`⌘ R W`, RB-030-04); MCP `wsl_info` with
/// `action: "restart"`.
pub const RESTART_HELPER: &str = "wsl.restart_helper";
/// The i18n key of the answer when the project is not watched through the WSL helper.
pub const NO_HELPER_KEY: &str = "wsl.error.noHelper";

/// The window's hot reload; one window per process.
static SERVICE: Mutex<Option<HotReload>> = Mutex::new(None);

fn current() -> Option<HotReload> {
    lock(&SERVICE).clone()
}

/// Runs the command `id` when it is this module's (`reload.*`, `wsl.restart_helper`); `None`
/// otherwise. A full `reload.all` is navigation sync's (RB-020-06: the coupled views), and
/// before [`on_start`] `reload.all` is left to the core's own handler.
pub fn run_command(core: &Core, id: &str, args: &Value) -> Option<Result<Value, CommandError>> {
    let _ = core;
    let forced = match id {
        RELOAD_ALL if ReloadArgs::is_full(args) => return None,
        RELOAD_ALL => None,
        RELOAD_CSS_ONLY => Some(ReloadKind::Css),
        TOGGLE_PAUSE => {
            let Some(service) = current() else {
                return Some(Err(not_started()));
            };
            return Some(Ok(json!({ "paused": service.toggle_pause() })));
        }
        RESTART_HELPER => {
            let Some(service) = current() else {
                return Some(Err(not_started()));
            };
            return Some(run_restart_helper(&service));
        }
        _ => return None,
    };
    let service = match current() {
        Some(service) => service,
        None if id == RELOAD_ALL => return None,
        None => return Some(Err(not_started())),
    };
    Some(run_reload(&service, args, forced))
}

fn run_reload(
    service: &HotReload,
    args: &Value,
    forced: Option<ReloadKind>,
) -> Result<Value, CommandError> {
    let mut args = ReloadArgs::from_value(args).map_err(CommandError::InvalidArgs)?;
    if let Some(mode) = forced {
        args.mode = mode;
    }
    let answer = service
        .block_on(service.reload(args))
        .ok_or_else(|| CommandError::Failed("reload.error.notOnRuntime".to_owned()))?;
    serde_json::to_value(answer).map_err(|error| CommandError::Failed(error.to_string()))
}

fn run_restart_helper(service: &HotReload) -> Result<Value, CommandError> {
    if service.restart_helper() {
        Ok(json!({ "restarted": true }))
    } else {
        Err(CommandError::Failed(NO_HELPER_KEY.to_owned()))
    }
}

fn not_started() -> CommandError {
    CommandError::Failed("reload.error.notStarted".to_owned())
}

/// The options of the project and the user settings in effect.
#[must_use]
pub fn options_of(core: &Core) -> (ReloadOptions, WslSettings) {
    let user = core.user_settings();
    let config = core.host().config();
    let options = ReloadOptions::from_config(&config.reload, &config.environments, &user.reload);
    // rb-config models `wsl`; rb-wsl reads the same section with its own type.
    let wsl = serde_json::to_value(&user.wsl).ok();
    (options, WslSettings::from_value(wsl.as_ref()))
}

/// Starts hot reload for the window's project on `runtime`, and follows the active
/// environment (its interval reload).
pub fn on_start(core: &Core, runtime: &Handle) {
    let host = core.host().clone();
    let (options, wsl) = options_of(core);
    let service = HotReload::start(
        host.clone(),
        runtime.clone(),
        host.project_root(),
        options,
        wsl,
    );
    service.set_environment(host.shell_state().environment);
    *lock(&SERVICE) = Some(service);
    let mut events = host.subscribe();
    let host = Arc::downgrade(&host);
    runtime.spawn(async move {
        loop {
            match events.recv().await {
                // A lagged receiver may have missed the change: look anyway.
                Ok(ViewEvent::StateChanged) | Err(RecvError::Lagged(_)) => {}
                Ok(_) => continue,
                Err(RecvError::Closed) => return,
            }
            let (Some(host), Some(service)) = (host.upgrade(), current()) else {
                return;
            };
            service.set_environment(host.shell_state().environment);
        }
    });
}

/// The project config or the user settings changed: the watch starts again with them.
pub fn on_config_changed(core: &Core) {
    if let Some(service) = current() {
        let (options, wsl) = options_of(core);
        service.apply(core.host().project_root(), options, wsl);
    }
}

/// Input in a view (the sync hub, RB-020-02): the interval reload waits.
pub fn note_input() {
    if let Some(service) = current() {
        service.note_input();
    }
}

/// The watcher's state for `project_info` (RB-030-01) and the toolbar; `None` before
/// [`on_start`].
#[must_use]
pub fn watcher_info() -> Option<WatcherInfo> {
    current().map(|service| service.info())
}

/// The window closes: the watch and the WSL helper stop.
pub fn on_shutdown() {
    lock(&SERVICE).take();
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
