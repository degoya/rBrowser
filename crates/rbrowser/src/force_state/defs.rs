//! The commands of "Force state" (RB-090-53) in the DevTools group (`D`), where Chrome's own
//! `:hov` lives: `view.force_state.set` (`Leader D H`, hover: the element a click picks, without
//! arguments) and `view.force_state.reset` (`Leader D Q`, every view without arguments). MCP
//! `views_force_state` runs both.

use std::sync::{Arc, Weak};

use rb_commands::{
    Binding, Chord, CommandDef, CommandError, CommandId, CommandRegistry, Group, Handler, Key,
    McpCoverage,
};
use serde_json::Value;

use crate::control::Core;

/// Forces states on an element.
pub const SET: &str = "view.force_state.set";
/// Takes them back.
pub const RESET: &str = "view.force_state.reset";
/// The MCP tool of both.
pub const TOOL: &str = "views_force_state";

/// Id, the key after `D`.
const ROWS: [(&str, char); 2] = [(SET, 'H'), (RESET, 'Q')];

/// The definitions; title key `commands.<id>`.
///
/// # Errors
///
/// An id the registry would refuse (a test holds them valid).
pub fn definitions() -> Result<Vec<CommandDef>, CommandError> {
    ROWS.iter()
        .map(|(id, key)| {
            Ok(CommandDef {
                id: CommandId::new(id)?,
                title_key: format!("commands.{id}"),
                group: Group::Devtools,
                default_bindings: vec![Binding::Leader(Chord {
                    keys: vec![Key::Char(Group::Devtools.leader_key()), Key::Char(*key)],
                })],
                mcp: McpCoverage::Covered(TOOL.to_owned()),
            })
        })
        .collect()
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
    let reset = id == RESET;
    Arc::new(move |args: Value| {
        let core = core
            .upgrade()
            .ok_or_else(|| CommandError::Failed("the window is closed".to_owned()))?;
        if reset {
            super::command::reset(&core, &args)
        } else {
            super::command::run(&core, &args)
        }
    })
}

#[cfg(test)]
#[path = "defs_tests.rs"]
mod tests;
