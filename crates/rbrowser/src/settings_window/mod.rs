//! The settings window (RB-010-08): its own CEF window with the shell's pages
//! (`cef_app/settings_window.rs`, `rbrowser://ui/settings.html`), the settings over IPC
//! (`getSettings` with the schema, `setSetting` with `settingRejected`, `settingsChanged` to
//! every page), deep links (`settings.open { key }`, `openSettings { key }`), the native path
//! dialog (`pickPath`), and who may query the core at all ([`guard`]).
//!
//! `pickPath` answers `ack` at once; the choice follows as the event `pickedPath`, because the
//! dialog runs on the UI thread the query is answered on. A field with `"x-options": "dynamic"`
//! gets its choices from the core with the schema ([`dynamic`], RB-080-32).

mod apply;
mod deepcheck;
mod docedit;
mod dynamic;
pub mod guard;
mod lane;
mod program;
mod project_edit;
mod refs;
pub mod user_file;
mod values;
pub mod write;

use rb_commands::CommandError;
use rb_commands::builtin::SHORTCUTS_EDIT_COMMAND;
use rb_ipc::{CoreToShell, ShellToCore};
use rb_settings::{Category, SettingsRegistry};
use serde_json::Value;

pub use apply::{
    PICK_TITLE_KEY, get_settings_off_thread, set_log_handle, set_setting, set_setting_off_thread,
};
pub use project_edit::{Edit, edit_project_file};
pub use write::Caller;

use crate::control::Core;
use crate::ipc::Failure;
use crate::shell::{ROUTE_SETTINGS, page_url};

/// The command that opens the settings.
pub const OPEN_COMMAND: &str = "settings.open";
/// The key the shortcut editor sits at (Keyboard Shortcuts > bindings).
pub const SHORTCUTS_KEY: &str = "shortcuts.bindings";
/// The command that opens the settings at the plugin list (RB-080-51).
pub const OPEN_PLUGINS_COMMAND: &str = "settings.open_plugins";
/// The category the plugin list stands in.
pub const PLUGINS_CATEGORY: &str = "plugins";

/// Runs the command `id` when it is this module's (`settings.open` with an optional `key`,
/// `shortcuts.edit`, `settings.open_plugins`); `None` otherwise.
pub fn run_command(core: &Core, id: &str, args: &Value) -> Option<Result<Value, CommandError>> {
    let key = match id {
        OPEN_COMMAND => args.get("key").and_then(Value::as_str),
        SHORTCUTS_EDIT_COMMAND => Some(SHORTCUTS_KEY),
        OPEN_PLUGINS_COMMAND => Some(PLUGINS_CATEGORY),
        _ => return None,
    };
    open(core, key);
    Some(Ok(Value::Null))
}

/// Answers `message` when it is this module's (`openSettings`, `getSettings`, `setSetting`,
/// `pickPath`, `getSettingSources`, `revealDiagnostics`); `None` otherwise.
pub fn ipc(core: &Core, message: &ShellToCore) -> Option<Result<CoreToShell, Failure>> {
    let answer = match message {
        ShellToCore::OpenSettings { key } => {
            open(core, key.as_deref());
            CoreToShell::Ack
        }
        ShellToCore::GetSettings { include_schema } => {
            apply::settings_answer(core, *include_schema)
        }
        ShellToCore::SetSetting { key, value, scope } => {
            set_setting(core, key, value, *scope, Caller::Shell)
        }
        ShellToCore::PickPath { kind, title_key } => {
            apply::pick_path(core, *kind, title_key.as_deref());
            CoreToShell::Ack
        }
        // The values of the `x-options` sources (RB-080-72), for the settings window only.
        ShellToCore::GetSettingSources => CoreToShell::SettingSources {
            sources: dynamic::sources(core),
        },
        // "Show in folder" after "Save diagnostics..." (RB-080-73): only the ZIP saved last.
        ShellToCore::RevealDiagnostics => {
            if !crate::control::diag_save::reveal_last() {
                return Some(Err((
                    rb_ipc::IpcErrorCode::NotFound,
                    "no diagnostics ZIP was saved".to_owned(),
                )));
            }
            CoreToShell::Ack
        }
        _ => return None,
    };
    Some(Ok(answer))
}

/// The project config was read again after a change: every page reads the project's settings
/// again (wired in `crate::features::on_config_changed`).
pub fn on_config_changed(core: &Core) {
    apply::project_changed(core);
}

/// Opens the settings window at `key` (a setting or a category id), else at its start.
pub fn open(core: &Core, key: Option<&str>) {
    let route = route(core.settings_registry(), key);
    core.ui().open_settings_window(page_url(&route));
}

/// The pages window's route for `key`: `settings/<key>` for a known setting or category,
/// `settings` for anything else (an unknown key never reaches the URL).
pub fn route(registry: &SettingsRegistry, key: Option<&str>) -> String {
    match key {
        Some(key) if registry.get(key).is_some() || Category::from_id(key).is_some() => {
            format!("{ROUTE_SETTINGS}/{key}")
        }
        Some(key) => {
            tracing::debug!(key, "no such setting; the settings open at their start");
            ROUTE_SETTINGS.to_owned()
        }
        None => ROUTE_SETTINGS.to_owned(),
    }
}

#[cfg(test)]
#[path = "settings_window_tests.rs"]
mod tests;
