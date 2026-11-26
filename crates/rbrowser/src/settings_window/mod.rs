//! The settings window (RB-010-08): its own CEF window with the shell's pages
//! (`cef_app/settings_window.rs`, `rbrowser://ui/settings.html`), the settings over IPC
//! (`getSettings` with the schema, `setSetting` with `settingRejected`, `settingsChanged` to
//! every page), deep links (`settings.open { key }`, `openSettings { key }`: a key, a category,
//! a section or a key prefix, [`target`], RB-090-85), the native path dialog (`pickPath`), and
//! who may query the core at all ([`guard`]).
//!
//! `pickPath` answers `ack` at once; the choice follows as the event `pickedPath`, because the
//! dialog runs on the UI thread the query is answered on. A field with `"x-options": "dynamic"`
//! gets its choices from the core with the schema ([`dynamic`], RB-080-32).

mod apply;
mod deepcheck;
mod docedit;
mod dynamic;
mod env_rename;
pub mod guard;
mod lane;
mod program;
mod project_edit;
mod refs;
pub mod target;
pub mod user_file;
mod values;
pub mod write;
mod write_order;

use rb_commands::CommandError;
use rb_commands::builtin::SHORTCUTS_EDIT_COMMAND;
use rb_ipc::{CoreToShell, ShellToCore};
use rb_settings::SettingsRegistry;
use serde_json::Value;

pub(crate) use apply::ONE_CHANGE;
pub use apply::{
    PICK_TITLE_KEY, change_off_thread, get_settings_off_thread, set_log_handle, set_setting,
    set_setting_off_thread,
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

/// Every key's value and origin now, a secret key included and masked: what MCP's
/// `settings_get` and its audit read (RB-094-27); the window's `getSettings` lists no secret.
pub fn mcp_values(core: &Core) -> Vec<rb_ipc::SettingValue> {
    values::all_values(&core.settings_registry(), &apply::documents(core))
}

/// The project config was read again after a change: every page reads the project's settings
/// again (wired in `crate::features::on_config_changed`).
pub fn on_config_changed(core: &Core) {
    apply::project_changed(core);
}

/// Opens the settings window at `key` (a setting, a category, a section or a key prefix,
/// [`target`]; a plugin's key once its plugin is loaded, RB-094-26), else at its start - with
/// the key named when it opens nothing: `pending` while an installed plugin of its prefix still
/// loads (the page asks again once they are in), else `missing` ([`target::not_found_query`]).
/// A secret key is no setting of the window: it opens the panel that edits it instead
/// ([`target::panel_of`], RB-094-27 - `api-client.secrets` the API client at Variables).
pub fn open(core: &Core, key: Option<&str>) {
    let registry = core.settings_registry();
    if let Some(panel) = key.and_then(|link| target::panel_of(&registry, link)) {
        crate::plugins::open::open_at(core, panel.plugin, panel.panel, panel.tab);
        return;
    }
    let route = match key {
        Some(link) if target::resolve(&registry, link).is_none() => {
            let loading =
                crate::analyzer::plugins::loaded().is_none() && !crate::analyzer::plugins::held();
            target::not_found_route(
                target::not_found_query(link, plugin_package_present, loading),
                link,
            )
        }
        _ => route(&registry, key),
    };
    core.ui().open_settings_window(page_url(&route));
}

/// Whether a package of plugin `id` lies in a folder this process loads plugins from (bundled,
/// the person's, the project's): `<id>.rbplugin` as an install names it, or
/// `<id>-<version>.rbplugin` as a release does. Read before the load, so by its name only.
fn plugin_package_present(id: &str) -> bool {
    let folders = [
        crate::analyzer::plugins::bundled_dir(),
        crate::analyzer::plugins::user_dir(),
        crate::analyzer::plugins::project_root()
            .map(|root| rb_plugin_host::project_plugins_dir(&root)),
    ];
    let versioned = format!("{id}-");
    folders.into_iter().flatten().any(|folder| {
        std::fs::read_dir(folder).is_ok_and(|entries| {
            entries.flatten().any(|entry| {
                let path = entry.path();
                path.extension()
                    .is_some_and(|ext| ext == rb_plugin_host::PACKAGE_EXTENSION)
                    && path
                        .file_stem()
                        .and_then(|stem| stem.to_str())
                        .is_some_and(|stem| {
                            stem == id
                                || stem.strip_prefix(&versioned).is_some_and(|version| {
                                    version.starts_with(|c: char| c.is_ascii_digit())
                                })
                        })
            })
        })
    })
}

/// The pages window's route for `key` ([`target::route_of`]); `settings` without one.
pub fn route(registry: &SettingsRegistry, key: Option<&str>) -> String {
    key.map_or_else(
        || ROUTE_SETTINGS.to_owned(),
        |key| target::route_of(registry, key),
    )
}

#[cfg(test)]
#[path = "settings_window_tests.rs"]
mod tests;
