//! The commands of the extensions for the one registry (RB-080-06), all in the plugins group:
//! `extensions.manage` (`Leader X M`) opens the person's folders in the settings,
//! `extensions.load_unpacked` (`Leader X O`) picks a folder and adds it, and
//! `extensions.add_recommended` (`Leader X A`) adds a trusted project's recommended folders
//! (and installs its Web Store ids), `extensions.install_web_store` (`Leader X W`) installs the
//! extension its `ref` argument names (an id or a store URL; without one it opens the field in
//! the settings), `extensions.check_updates` (`Leader X C`) asks the Web Store for newer
//! versions (RB-080-23) and `extensions.remove` (`Leader X X`) removes the Web Store extension
//! its `ref` argument names (without one it opens the list in the settings; refused while the
//! Web Store is off, like the install; RB-080-30). Adding
//! loads code into every view, so it is the person's command and no MCP tool runs one; MCP
//! reads the state with `extension_list`, installs only through `extension_install` and removes
//! only through `extension_remove`, both of which wait for the person's word in the window.
//! Every command answers at once; the dialog, the download and the write follow and end in a
//! notification.

use std::sync::{Arc, Weak};

use rb_commands::{
    Binding, Chord, CommandDef, CommandError, CommandId, CommandRegistry, Group, Handler, Key,
    McpCoverage,
};
use rb_ipc::{NotificationLevel, PathKind};
use serde_json::{Value, json};

use super::app::{
    ADDED_KEY, REFUSED_KEY, UNPACKED_KEY, add_folder, add_recommended, notice, recommended_notice,
    status,
};
use super::{remove, web};
use crate::control::Core;
use crate::host::ui::PathPick;

/// Open the person's extension folders in the settings.
pub const MANAGE: &str = "extensions.manage";
/// Pick an unpacked extension's folder and add it.
pub const LOAD_UNPACKED: &str = "extensions.load_unpacked";
/// Add the trusted project's recommended folders.
pub const ADD_RECOMMENDED: &str = "extensions.add_recommended";
/// Install an extension from the Web Store (`ref`: an id or a store URL).
pub const INSTALL_WEB_STORE: &str = "extensions.install_web_store";
/// Ask the Web Store for newer versions of the installed extensions.
pub const CHECK_UPDATES: &str = "extensions.check_updates";
/// Remove an extension installed from the Web Store (`ref`: an id or a store URL).
pub const REMOVE: &str = "extensions.remove";
/// The MCP tool that reads the state.
pub const TOOL_NAME: &str = "extension_list";
/// Why `extensions.manage` has no MCP tool.
pub const MANAGE_OMITTED: &str =
    "opens the settings window only; extension_list reads the extensions";
/// Why the adding commands have no MCP tool.
pub const INSTALL_OMITTED: &str =
    "protected: loads an extension's code into every view; only the person adds one";
/// Why the Web Store install command has no MCP tool of its own.
pub const WEB_STORE_OMITTED: &str = "protected: installs code into every view on the person's \
                                     word alone; MCP installs with extension_install, which \
                                     waits for the person's word in the window";
/// Why the update check has no MCP tool.
pub const UPDATES_OMITTED: &str = "every update found is a question in the window the person \
                                   answers; the start checks too (checkUpdatesAtStart)";
/// The title of the folder dialog.
pub const PICK_TITLE_KEY: &str = "extensions.pick.title";

/// The commands: id, the keys after the leader, the MCP tool or why there is none.
const ROWS: [(&str, [char; 2], Result<&str, &str>); 6] = [
    (MANAGE, ['X', 'M'], Err(MANAGE_OMITTED)),
    (LOAD_UNPACKED, ['X', 'O'], Err(INSTALL_OMITTED)),
    (ADD_RECOMMENDED, ['X', 'A'], Err(INSTALL_OMITTED)),
    (INSTALL_WEB_STORE, ['X', 'W'], Err(WEB_STORE_OMITTED)),
    (CHECK_UPDATES, ['X', 'C'], Err(UPDATES_OMITTED)),
    (REMOVE, ['X', 'X'], Ok(web::REMOVE_TOOL)),
];

/// The definitions; title key `commands.<id>`.
pub fn definitions() -> Result<Vec<CommandDef>, CommandError> {
    ROWS.iter()
        .map(|(id, keys, mcp)| {
            Ok(CommandDef {
                id: CommandId::new(id)?,
                title_key: format!("commands.{id}"),
                group: Group::Plugins,
                default_bindings: vec![Binding::Leader(Chord {
                    keys: keys.iter().map(|key| Key::Char(*key)).collect(),
                })],
                mcp: match mcp {
                    Ok(tool) => McpCoverage::Covered((*tool).to_owned()),
                    Err(reason) => McpCoverage::Omitted((*reason).to_owned()),
                },
            })
        })
        .collect()
}

/// Registers the commands with `registry`; returns what the registry refused (a test holds it
/// empty).
pub fn register(registry: &mut CommandRegistry, core: &Arc<Core>) -> Vec<CommandError> {
    let defs = match definitions() {
        Ok(defs) => defs,
        Err(error) => return vec![error],
    };
    defs.into_iter()
        .filter_map(|def| {
            let handler = handler(def.id.as_str(), Arc::downgrade(core));
            registry.register(def, handler).err()
        })
        .collect()
}

fn handler(id: &str, core: Weak<Core>) -> Handler {
    let id = id.to_owned();
    Arc::new(move |args: Value| {
        let core = core
            .upgrade()
            .ok_or_else(|| CommandError::Failed("the window is closed".to_owned()))?;
        run(&core, &id, &args)
    })
}

fn run(core: &Arc<Core>, id: &str, args: &Value) -> Result<Value, CommandError> {
    match id {
        MANAGE => {
            crate::settings_window::open(core, Some(UNPACKED_KEY));
            Ok(json!({ "opened": true }))
        }
        LOAD_UNPACKED => {
            let me = Arc::downgrade(core);
            let pick = PathPick {
                kind: PathKind::Folder,
                title: rb_i18n::t(PICK_TITLE_KEY),
                save_as: None,
            };
            core.ui().pick_path(
                pick,
                Box::new(move |path| {
                    let (Some(path), Some(core)) = (path, me.upgrade()) else {
                        return;
                    };
                    let worker = core.clone();
                    core.runtime().spawn_blocking(move || {
                        let message = match add_folder(&worker, &path.to_string_lossy()) {
                            Ok(name) => {
                                notice(NotificationLevel::Info, ADDED_KEY, &[("name", name)])
                            }
                            Err(error) => {
                                notice(NotificationLevel::Error, REFUSED_KEY, &[("error", error)])
                            }
                        };
                        worker.notify(message);
                    });
                }),
            );
            Ok(json!({ "started": true }))
        }
        ADD_RECOMMENDED => {
            let worker = core.clone();
            core.runtime().spawn_blocking(move || {
                let message = match add_recommended(&worker) {
                    Ok(done) => recommended_notice(&done),
                    Err(error) => {
                        notice(NotificationLevel::Error, REFUSED_KEY, &[("error", error)])
                    }
                };
                worker.notify(message);
            });
            Ok(json!({ "started": true }))
        }
        INSTALL_WEB_STORE => {
            let Some(written) = web::ref_arg(args, "ref") else {
                crate::settings_window::open(core, Some(web::ALLOW_KEY));
                return Ok(json!({ "opened": true }));
            };
            let worker = core.clone();
            core.runtime().spawn_blocking(move || {
                let message = match web::install_by_ref(&worker, &written) {
                    Ok(installed) => {
                        remove::push_list(&worker);
                        web::notice(
                            NotificationLevel::Info,
                            web::INSTALLED_KEY,
                            &[("name", installed.name), ("version", installed.version)],
                        )
                    }
                    Err(error) => web::notice(
                        NotificationLevel::Error,
                        web::FAILED_KEY,
                        &[("id", written), ("error", error)],
                    ),
                };
                worker.notify(message);
            });
            Ok(json!({ "started": true }))
        }
        CHECK_UPDATES => {
            let worker = core.clone();
            core.runtime()
                .spawn_blocking(move || web::check_updates(&worker, true));
            Ok(json!({ "started": true }))
        }
        REMOVE => {
            let Some(written) = web::ref_arg(args, "ref") else {
                crate::settings_window::open(core, Some(web::ALLOW_KEY));
                return Ok(json!({ "opened": true }));
            };
            let worker = core.clone();
            core.runtime().spawn_blocking(move || {
                let message = match remove::remove_by_ref(&worker, &written) {
                    Ok(removed) => {
                        remove::push_list(&worker);
                        remove::removed_notice(&removed)
                    }
                    Err(error) => remove::failed_notice(&written, error),
                };
                worker.notify(message);
            });
            Ok(json!({ "started": true }))
        }
        other => Err(CommandError::Failed(format!(
            "no extensions command {other}"
        ))),
    }
}

/// `extension_list`'s arguments: none.
///
/// # Errors
///
/// Arguments that are not an empty object.
pub fn tool_args(args: &Value) -> Result<(), rb_mcp::ToolFailure> {
    match args {
        Value::Null => Ok(()),
        Value::Object(map) if map.is_empty() => Ok(()),
        _ => Err(rb_mcp::ToolFailure::new(
            "args.invalid",
            "extension_list takes no arguments",
        )),
    }
}

/// The MCP tool `extension_list`: the state ([`status`]), read off the async workers (it reads
/// each folder's manifest). It installs and removes nothing.
///
/// # Errors
///
/// Arguments, or the read did not finish.
pub async fn host_tool(core: &Arc<Core>, args: Value) -> Result<Value, rb_mcp::ToolFailure> {
    tool_args(&args)?;
    let reader = core.clone();
    tokio::task::spawn_blocking(move || status(&reader))
        .await
        .map_err(|error| rb_mcp::ToolFailure::new("mcp.internal", error.to_string()))
}

#[cfg(test)]
#[path = "defs_tests.rs"]
mod tests;
