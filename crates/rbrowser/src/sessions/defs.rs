//! The commands of sessions for the one registry (RB-020-07): ids, group, default leader chords
//! and MCP tools. They sit in the Environments group (`E`, the wiki's "sessions and
//! environments"): `Leader V S` belongs to the free size input of RB-010-16 and `Leader V K` is
//! planned for WebKit (RB-040-04).

use std::sync::{Arc, Weak};

use rb_commands::{
    Binding, Chord, CommandDef, CommandError, CommandId, CommandRegistry, Group, Handler, Key,
    McpCoverage,
};
use serde_json::Value;

use super::mcp::{ASSIGN_TOOL, CLEAR_TOOL};
use crate::control::Core;

/// Move the active view (or `target`) into a session; without `session` the shell's picker
/// opens.
pub const ASSIGN: &str = "session.assign";
/// Clear a session's data; without `session` the shell asks which one, and to confirm.
pub const CLEAR: &str = "session.clear";

/// Id, the keys after the leader, MCP tool.
const ROWS: [(&str, [char; 2], &str); 2] = [
    (ASSIGN, ['E', 'S'], ASSIGN_TOOL),
    (CLEAR, ['E', 'K'], CLEAR_TOOL),
];

/// The definitions; title key `commands.<id>`.
pub fn definitions() -> Result<Vec<CommandDef>, CommandError> {
    ROWS.iter()
        .map(|(id, keys, tool)| {
            Ok(CommandDef {
                id: CommandId::new(id)?,
                title_key: format!("commands.{id}"),
                group: Group::Environments,
                default_bindings: vec![Binding::Leader(Chord {
                    keys: keys.iter().map(|key| Key::Char(*key)).collect(),
                })],
                mcp: McpCoverage::Covered((*tool).to_owned()),
            })
        })
        .collect()
}

/// Whether `id` is a command of this module.
pub fn is_own(id: &str) -> bool {
    ROWS.iter().any(|(own, _, _)| *own == id)
}

/// Registers both commands with `registry`, each running [`super::run`]; returns what the
/// registry refused (a test holds it empty).
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
        super::run(&core, &id, &args)
    })
}

#[cfg(test)]
#[path = "defs_tests.rs"]
mod tests;
