//! The commands of the status bar (RB-080-15) in the Panels group (`P`): `status.toggle_bar`
//! (`Leader P S`) switches `ui.statusBar.visible`, `status.history` (`Leader P L`, the log)
//! opens the history above the bar. Neither has a tool: the switch is a setting
//! (`settings_set`), and `status_messages` reads the history itself.

use std::sync::{Arc, Weak};

use rb_commands::{
    Binding, Chord, CommandDef, CommandError, CommandId, CommandRegistry, Group, Handler, Key,
    McpCoverage,
};
use rb_ipc::CoreToShell;
use serde_json::{Value, json};

use crate::control::Core;
use crate::page_window::PageWindow;
use crate::settings_window::Caller;

/// Show or hide the status bar.
pub const TOGGLE_BAR: &str = "status.toggle_bar";
/// Open the message history.
pub const HISTORY: &str = "status.history";
/// The setting `status.toggle_bar` switches.
pub const VISIBLE_KEY: &str = "ui.statusBar.visible";
/// Why `status.toggle_bar` has no tool.
pub const TOGGLE_OMITTED: &str = "display switch; settings_set writes ui.statusBar.visible";
/// Why `status.history` has no tool.
pub const HISTORY_OMITTED: &str = "display only; status_messages reads the messages";

/// Id, the key after `P`, why no tool.
const ROWS: [(&str, char, &str); 2] = [
    (TOGGLE_BAR, 'S', TOGGLE_OMITTED),
    (HISTORY, 'L', HISTORY_OMITTED),
];

/// The definitions; title key `commands.<id>`.
///
/// # Errors
///
/// An id the registry would refuse (a test holds them valid).
pub fn definitions() -> Result<Vec<CommandDef>, CommandError> {
    ROWS.iter()
        .map(|(id, key, omitted)| {
            Ok(CommandDef {
                id: CommandId::new(id)?,
                title_key: format!("commands.{id}"),
                group: Group::Panels,
                default_bindings: vec![Binding::Leader(Chord {
                    keys: vec![Key::Char(Group::Panels.leader_key()), Key::Char(*key)],
                })],
                mcp: McpCoverage::Omitted((*omitted).to_owned()),
            })
        })
        .collect()
}

/// Whether `id` is a command of this module.
#[must_use]
pub fn is_own(id: &str) -> bool {
    ROWS.iter().any(|(own, _, _)| *own == id)
}

/// Registers both commands with `registry`; returns what the registry refused.
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
    Arc::new(move |_args: Value| {
        let core = core
            .upgrade()
            .ok_or_else(|| CommandError::Failed("the window is closed".to_owned()))?;
        run(&core, &id)
    })
}

/// Runs `status.toggle_bar` or `status.history`.
///
/// # Errors
///
/// An unknown id, or the setting could not be written.
pub fn run(core: &Core, id: &str) -> Result<Value, CommandError> {
    match id {
        TOGGLE_BAR => toggle_bar(core),
        HISTORY => {
            let history = PageWindow::History;
            core.ui().open_page_window(history, history.url());
            // The bar counts the messages as read.
            core.send(&CoreToShell::UiCommand { id: id.to_owned() });
            Ok(Value::Null)
        }
        other => Err(CommandError::UnknownCommand(other.to_owned())),
    }
}

/// Writes the switched `ui.statusBar.visible`; the window follows the setting.
fn toggle_bar(core: &Core) -> Result<Value, CommandError> {
    let visible = !core.user_settings().ui.status_bar.visible;
    let answer = crate::settings_window::set_setting(
        core,
        VISIBLE_KEY,
        &Value::Bool(visible),
        rb_settings::Scope::User,
        Caller::Shell,
    );
    if let CoreToShell::SettingRejected { .. } = answer {
        return Err(CommandError::Failed(format!(
            "{VISIBLE_KEY} could not be written"
        )));
    }
    Ok(json!({ "visible": visible }))
}

#[cfg(test)]
#[path = "defs_tests.rs"]
mod tests;
