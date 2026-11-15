//! DevTools (RB-010-06): docked (the hosted frontend over an internal one-time endpoint) or
//! in a window, following the active view, the dock layout, re-applying the emulation after
//! DevTools' device toolbar. The CEF side is `cef_app/devtools.rs` and the dock area in
//! `cef_app/window.rs`; the inspected view is [`crate::host::shell_state::ShellState::devtools`].
//!
//! | Part | Module |
//! | --- | --- |
//! | One window's DevTools: open, close, follow, the dock's preparation | [`session`] |
//! | The internal WebSocket endpoint and its one-time tickets | [`endpoint`], [`token`] |
//! | What passes between frontend and browser | [`relay`] |
//! | The hosted frontend's screencast, off unless `devtools.screencast` | [`screencast`] |
//! | "Inspect element": `devtools.inspect_at`, the node under a point revealed in the frontend | [`inspect`] |
//! | Targets no outer CDP client sees | [`hidden`] |
//! | Dock sides, the mode cycle, the dock's size | [`layout`] |
//! | The splitter: its page, the drag, the size kept as a setting | [`splitter`] |
//! | The window bounds in `devtools.json` | [`ui_state`] |
//! | Which keyboard focus a key comes from (`shortcuts.disableInDevtools`) | [`focus`] |

pub mod endpoint;
pub mod focus;
pub mod hidden;
pub mod inspect;
pub mod layout;
pub mod relay;
pub mod screencast;
pub mod session;
pub mod splitter;
pub mod token;
pub mod ui_state;

use std::sync::{Arc, Mutex, Weak};

use rb_commands::CommandError;
use rb_config::DevtoolsMode;
use rb_engine::{EngineError, ViewId};
use serde_json::{Value, json};
use tokio::runtime::Handle;

use crate::cdp::lock;
use crate::control::Core;
use crate::host::Host;
pub use session::Devtools;
pub use ui_state::WindowBounds;

/// The DevTools of every live window, by host.
fn registry() -> &'static Mutex<Vec<(Weak<Host>, Arc<Devtools>)>> {
    static REGISTRY: Mutex<Vec<(Weak<Host>, Arc<Devtools>)>> = Mutex::new(Vec::new());
    &REGISTRY
}

/// The DevTools of the window of `host`, once [`on_start`] ran for it.
pub fn for_host(host: &Host) -> Option<Arc<Devtools>> {
    let mut entries = lock(registry());
    entries.retain(|(weak, _)| weak.strong_count() > 0);
    entries
        .iter()
        .find(|(weak, _)| std::ptr::eq(weak.as_ptr(), host))
        .map(|(_, devtools)| devtools.clone())
}

/// The DevTools of `core`'s window.
fn of(core: &Core) -> Result<Arc<Devtools>, EngineError> {
    for_host(core.host()).ok_or_else(|| EngineError::Invalid("DevTools are not started".to_owned()))
}

/// Runs the command `id` when it is this module's (`devtools.*`); `None` otherwise.
pub fn run_command(core: &Core, id: &str, args: &Value) -> Option<Result<Value, CommandError>> {
    let result = match id {
        "devtools.toggle" => toggle(core),
        "devtools.open" => open_command(core, args),
        "devtools.dock_cycle" => dock_cycle(core),
        inspect::COMMAND => inspect::run(core, args),
        _ => return None,
    };
    Some(result)
}

fn failed(error: EngineError) -> CommandError {
    CommandError::Failed(error.to_string())
}

fn state_json(view: ViewId, mode: DevtoolsMode) -> Value {
    json!({ "open": true, "view": view, "mode": mode })
}

fn toggle(core: &Core) -> Result<Value, CommandError> {
    let devtools = of(core).map_err(failed)?;
    Ok(match devtools.toggle().map_err(failed)? {
        Some(open) => state_json(open.view, open.mode),
        None => json!({ "open": false }),
    })
}

/// `devtools.open { target?: viewId, mode? }` (also `viewId`); the active view without one.
fn open_command(core: &Core, args: &Value) -> Result<Value, CommandError> {
    let devtools = of(core).map_err(failed)?;
    let view = match args.get("target").or_else(|| args.get("viewId")) {
        None | Some(Value::Null) => {
            use rb_engine::ViewHost;
            let host = core.host();
            host.active_view()
                .or_else(|| host.views().first().map(|info| info.id))
                .ok_or_else(|| CommandError::Failed("there is no view".to_owned()))?
        }
        Some(value) => value
            .as_u64()
            .and_then(|id| u32::try_from(id).ok())
            .map(ViewId)
            .ok_or_else(|| {
                CommandError::InvalidArgs(
                    "`target` is one view id (DevTools inspect one view)".to_owned(),
                )
            })?,
    };
    let mode = match args.get("mode") {
        None | Some(Value::Null) => devtools.configured_mode(),
        Some(value) => serde_json::from_value(value.clone()).map_err(|_| {
            CommandError::InvalidArgs("`mode` is docked-bottom, docked-right or window".to_owned())
        })?,
    };
    devtools.open(view, mode).map_err(failed)?;
    Ok(state_json(view, mode))
}

/// `devtools.dock_cycle`: the next mode becomes the setting (kept in the settings file) and
/// open DevTools move there.
fn dock_cycle(core: &Core) -> Result<Value, CommandError> {
    let devtools = of(core).map_err(failed)?;
    let mut user = core.user_settings();
    let mode = layout::next_mode(user.devtools.mode);
    user.devtools.mode = mode;
    core.replace_user_settings(user);
    if let Some(path) = core.user_settings_path() {
        persist_mode(path, mode);
    }
    devtools.on_settings_changed().map_err(failed)?;
    Ok(json!({ "mode": mode }))
}

/// Writes `devtools.mode` into the user settings file at `path`, keeping the rest; a file that
/// does not read or comes from a newer rBrowser is left alone
/// ([`crate::settings_window::user_file::update`]).
pub fn persist_mode(path: &std::path::Path, mode: DevtoolsMode) {
    let written = crate::settings_window::user_file::update(path, |root| {
        let section = root.entry("devtools").or_insert_with(|| json!({}));
        if !section.is_object() {
            *section = json!({});
        }
        section["mode"] = json!(mode);
    });
    if let Err(error) = written {
        tracing::warn!(%error, "devtools.mode was not saved");
    }
}

/// [`rb_engine::ViewHost::open_devtools`] (MCP `devtools_open` and the commands).
pub fn open_devtools(host: &Host, view: ViewId, mode: DevtoolsMode) -> Result<(), EngineError> {
    for_host(host)
        .ok_or_else(|| EngineError::Invalid("DevTools are not started".to_owned()))?
        .open(view, mode)
}

/// Starts DevTools for `core`'s window: following the active view
/// (`devtools.followActiveView`) and closed views. The UI state is kept only where the user
/// settings are (a core without a settings file keeps nothing, like the tests).
pub fn on_start(core: &Core) {
    let state_path = core
        .user_settings_path()
        .and_then(|_| ui_state::default_path());
    // The core's runtime, not `Handle::current()`: CEF calls this on its UI thread, which is
    // outside the Tokio runtime.
    start(core, core.runtime().clone(), state_path);
}

/// [`on_start`] with the runtime and the UI state's file given.
pub fn start(
    core: &Core,
    runtime: Handle,
    state_path: Option<std::path::PathBuf>,
) -> Arc<Devtools> {
    let devtools = Devtools::new(core, runtime, state_path);
    devtools.follow(core.host());
    let mut entries = lock(registry());
    entries.retain(|(weak, _)| weak.strong_count() > 0);
    entries.push((Arc::downgrade(core.host()), devtools.clone()));
    devtools
}

/// The user settings changed (the settings window, RB-010-08): open DevTools follow
/// `devtools.mode` and `devtools.screencast`.
pub fn on_settings_changed(core: &Core) {
    if let Some(devtools) = for_host(core.host())
        && let Err(error) = devtools.on_settings_changed()
    {
        tracing::warn!(%error, "DevTools did not move to the new mode");
    }
}

/// The dock (CEF side) finished loading `url`.
pub fn on_dock_loaded(core: &Core, url: &str) {
    if let Some(devtools) = for_host(core.host()) {
        devtools.on_dock_loaded(url);
    }
}

/// The DevTools window of `view` closed at `bounds` (CEF side).
pub fn on_window_closed(core: &Core, view: ViewId, bounds: Option<WindowBounds>) {
    if let Some(devtools) = for_host(core.host()) {
        devtools.on_window_closed(view, bounds);
    }
}

/// Where the DevTools window opens (CEF side): its last usable bounds.
pub fn window_bounds(core: &Core) -> Option<WindowBounds> {
    for_host(core.host()).and_then(|devtools| devtools.window_bounds())
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "inspect_setting_tests.rs"]
mod inspect_setting_tests;
