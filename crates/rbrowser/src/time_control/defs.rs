//! The commands of the page clock and the animation rates (RB-094-07): `emulation.clock`
//! (`Leader S T`: one clock for every view, as the sync keeps them alike) and
//! `overlays.animations` (`Leader O J`: `O A` is the target-size overlay's). MCP `clock_set`
//! and `animations_set` run them.

use std::sync::{Arc, Weak};

use rb_commands::{
    Binding, Chord, CommandDef, CommandError, CommandId, CommandRegistry, Group, Handler, Key,
    McpCoverage,
};
use serde_json::Value;

use crate::control::Core;

/// Sets or resets the page clock of every view.
pub const CLOCK: &str = "emulation.clock";
/// Sets the animation rate or steps stopped animations.
pub const ANIMATIONS: &str = "overlays.animations";
/// The MCP tool of the clock.
pub const CLOCK_TOOL: &str = "clock_set";
/// The MCP tool of the animations.
pub const ANIMATIONS_TOOL: &str = "animations_set";

/// Id, group, the key after the group's, MCP tool.
const ROWS: [(&str, Group, char, &str); 2] = [
    (CLOCK, Group::Sync, 'T', CLOCK_TOOL),
    (ANIMATIONS, Group::Overlays, 'J', ANIMATIONS_TOOL),
];

/// The definitions; title key `commands.<id>`.
///
/// # Errors
///
/// An id the registry would refuse (a test holds them valid).
pub fn definitions() -> Result<Vec<CommandDef>, CommandError> {
    ROWS.iter()
        .map(|(id, group, key, tool)| {
            Ok(CommandDef {
                id: CommandId::new(id)?,
                title_key: format!("commands.{id}"),
                group: *group,
                default_bindings: vec![Binding::Leader(Chord {
                    keys: vec![Key::Char(group.leader_key()), Key::Char(*key)],
                })],
                mcp: McpCoverage::Covered((*tool).to_owned()),
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
    let clock = id == CLOCK;
    Arc::new(move |args: Value| {
        let core = core
            .upgrade()
            .ok_or_else(|| CommandError::Failed("the window is closed".to_owned()))?;
        if clock {
            super::command::clock(&core, &args)
        } else {
            super::command::animations(&core, &args)
        }
    })
}

#[cfg(test)]
#[path = "defs_tests.rs"]
mod tests;
