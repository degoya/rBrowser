//! The commands of a session's login state for the one registry (RB-094-08):
//! `session.state_export` (`Leader E W`, write) and `session.state_import` (`Leader E I`), in
//! the Environments group beside `session.assign` and `session.clear`; a person's, never MCP's.

use std::sync::{Arc, Weak};

use rb_commands::{
    Binding, Chord, CommandDef, CommandError, CommandId, CommandRegistry, Group, Handler, Key,
    McpCoverage,
};
use serde_json::Value;

use crate::basic_auth::SECRETS_OMITTED;
use crate::control::Core;

/// Write the login state of a session to `.rbrowser/out/state/<session>.json`.
pub const EXPORT: &str = "session.state_export";
/// Load a login state file into a session.
pub const IMPORT: &str = "session.state_import";

/// Id and the keys after the leader.
const ROWS: [(&str, [char; 2]); 2] = [(EXPORT, ['E', 'W']), (IMPORT, ['E', 'I'])];

/// The definitions; title key `commands.<id>`.
pub fn definitions() -> Result<Vec<CommandDef>, CommandError> {
    ROWS.iter()
        .map(|(id, keys)| {
            Ok(CommandDef {
                id: CommandId::new(id)?,
                title_key: format!("commands.{id}"),
                group: Group::Environments,
                default_bindings: vec![Binding::Leader(Chord {
                    keys: keys.iter().map(|key| Key::Char(*key)).collect(),
                })],
                mcp: McpCoverage::Omitted(SECRETS_OMITTED.to_owned()),
            })
        })
        .collect()
}

/// Refuses both commands to an MCP client.
pub fn refuse_mcp(id: &str) -> Result<(), CommandError> {
    if ROWS.iter().any(|(own, _)| *own == id) {
        return Err(CommandError::Failed(format!("{id}: {SECRETS_OMITTED}")));
    }
    Ok(())
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
