//! The commands of the grid (RB-010-04): their definitions for the command registry
//! (RB-010-17) and what they do. Each is reachable through a leader chord; none has an MCP tool
//! of its own (RB-030-01): zoom and keyboard focus are the person's view of the grid, and the
//! config reloads by itself when its file changes (`config_get` reads it).

use std::sync::Arc;

use rb_commands::{
    Binding, Chord, CommandDef, CommandError, CommandId, CommandRegistry, Group, Handler, Key,
    McpCoverage,
};
use serde_json::{Value, json};

use crate::host::Host;

/// Why the zoom commands have no MCP tool.
const ZOOM_OMITTED: &str = "ui-only: grid zoom";
/// Why the focus commands have no MCP tool.
const FOCUS_OMITTED: &str = "ui-only: keyboard focus between views";

/// `(id, group, keys after the leader, why no MCP tool)` of every command. `V F`, `V N` and
/// `V P` belong to `view.forward`, `view.navigate` and the palette (`rb_commands::builtin`).
const COMMANDS: [(&str, Group, [char; 2], &str); 6] = [
    ("view.zoom_fit_all", Group::View, ['V', 'Z'], ZOOM_OMITTED),
    ("view.zoom_in", Group::View, ['V', 'I'], ZOOM_OMITTED),
    ("view.zoom_out", Group::View, ['V', 'O'], ZOOM_OMITTED),
    ("view.focus_next", Group::View, ['V', ']'], FOCUS_OMITTED),
    ("view.focus_prev", Group::View, ['V', '['], FOCUS_OMITTED),
    (
        "app.reload_config",
        Group::Settings,
        [',', 'R'],
        "ui-only: the watcher reloads the config; config_get reads it",
    ),
];

/// The definitions: title key `commands.<id>`, one leader chord each.
pub fn definitions() -> Result<Vec<CommandDef>, CommandError> {
    COMMANDS
        .iter()
        .map(|(id, group, keys, omitted)| {
            Ok(CommandDef {
                id: CommandId::new(id)?,
                title_key: format!("commands.{id}"),
                group: *group,
                default_bindings: vec![Binding::Leader(Chord {
                    keys: keys.iter().map(|key| Key::Char(*key)).collect(),
                })],
                mcp: McpCoverage::Omitted((*omitted).to_owned()),
            })
        })
        .collect()
}

/// Runs the command `id` on `host`.
pub fn run(host: &Host, id: &str, _args: Value) -> Result<Value, CommandError> {
    let failed = |error: rb_engine::EngineError| CommandError::Failed(error.to_string());
    match id {
        "view.zoom_fit_all" => host.zoom_fit_all(),
        "view.zoom_in" => host.zoom_active(true).map_err(failed)?,
        "view.zoom_out" => host.zoom_active(false).map_err(failed)?,
        "view.focus_next" => return Ok(json!({ "view": host.focus_step(true).map_err(failed)? })),
        "view.focus_prev" => return Ok(json!({ "view": host.focus_step(false).map_err(failed)? })),
        "app.reload_config" => reload_config(host).map_err(CommandError::Failed)?,
        other => return Err(CommandError::UnknownCommand(other.to_owned())),
    }
    Ok(Value::Null)
}

/// Reads the project config again and applies it; views whose cell keeps its breakpoint stay.
/// The files read are remembered as applied (`project_reload::reload`, RB-094-23).
pub fn reload_config(host: &Host) -> Result<(), String> {
    crate::project_reload::reload(host)
}

/// Registers every command with `registry`; returns what the registry refused.
pub fn register(registry: &mut CommandRegistry, host: &Arc<Host>) -> Vec<CommandError> {
    let definitions = match definitions() {
        Ok(definitions) => definitions,
        Err(error) => return vec![error],
    };
    definitions
        .into_iter()
        .filter_map(|def| {
            let weak = Arc::downgrade(host);
            let id = def.id.as_str().to_owned();
            let handler: Handler = Arc::new(move |args| {
                let host = weak
                    .upgrade()
                    .ok_or_else(|| CommandError::Failed("the window is closed".to_owned()))?;
                run(&host, &id, args)
            });
            registry.register(def, handler).err()
        })
        .collect()
}

#[cfg(test)]
#[path = "commands_tests.rs"]
mod tests;
