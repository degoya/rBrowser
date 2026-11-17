//! The dock's commands for the one registry (RB-080-57), in the Panels group: the arrows after
//! `Leader P` point where the panels go - `P Left`, `P Right`, `P Down` dock them, `P Up` lifts
//! them into their own window - `P X` closes them (to the icon rail, or hidden, RB-080-93) and
//! `P Z` folds them to the rail and opens them again (RB-080-61; Z as in an editor's fold; every letter of `P` but Q, U and V is taken by
//! the core and the bundled plugins). Each takes `{plugin?, panel?}` (the panel to show), the
//! dock commands also `{size?}` or `{step?}` (DIP, kept for the side) and `{collapsed?}`, the
//! toggle `{collapsed?}` (sets instead of flipping), the close `{to?}` (`rail` or `hidden`). MCP
//! runs them all through `panels_dock`.

use std::sync::{Arc, Weak};

use rb_commands::{
    Binding, Chord, CommandDef, CommandError, CommandId, CommandRegistry, Group, Handler, Key,
    McpCoverage,
};
use serde_json::Value;

use super::PanelSide;
use crate::control::Core;

/// Dock the panels left of the views.
pub const DOCK_LEFT: &str = "panels.dock.left";
/// Dock the panels right of the views.
pub const DOCK_RIGHT: &str = "panels.dock.right";
/// Dock the panels below the views.
pub const DOCK_BOTTOM: &str = "panels.dock.bottom";
/// The panels in a window of their own again.
pub const UNDOCK: &str = "panels.undock";
/// Close the panels: to their icon rail or hidden, as `panels.closeTo` or `{to}` says
/// (RB-080-93).
pub const CLOSE: &str = "panels.close";
/// Fold the docked panels to a strip, or open them again (RB-080-61).
pub const TOGGLE: &str = "panels.dock.toggle";
/// The MCP tool behind every one of them.
pub const TOOL: &str = "panels_dock";

/// Id and the key after `Leader P`.
const ROWS: [(&str, Key); 6] = [
    (DOCK_LEFT, Key::Left),
    (DOCK_RIGHT, Key::Right),
    (DOCK_BOTTOM, Key::Down),
    (UNDOCK, Key::Up),
    (CLOSE, Key::Char('X')),
    (TOGGLE, Key::Char('Z')),
];

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
                group: Group::Panels,
                default_bindings: vec![Binding::Leader(Chord {
                    keys: vec![Key::Char(Group::Panels.leader_key()), *key],
                })],
                mcp: McpCoverage::Covered(TOOL.to_owned()),
            })
        })
        .collect()
}

/// Registers the commands with `registry`; returns what the registry refused.
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

/// Runs the dock command `id` with `args`.
///
/// # Errors
///
/// A bad size, step, collapsed or to, or an id that is not this module's.
pub fn run(core: &Core, id: &str, args: &Value) -> Result<Value, CommandError> {
    match id {
        DOCK_LEFT => super::dock(core, PanelSide::Left, args),
        DOCK_RIGHT => super::dock(core, PanelSide::Right, args),
        DOCK_BOTTOM => super::dock(core, PanelSide::Bottom, args),
        UNDOCK => super::undock(core, args),
        CLOSE => super::close(core, args),
        TOGGLE => super::collapse::toggle(core, args),
        other => Err(CommandError::UnknownCommand(other.to_owned())),
    }
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

#[cfg(test)]
#[path = "defs_tests.rs"]
mod tests;
