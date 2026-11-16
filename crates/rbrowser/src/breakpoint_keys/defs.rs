//! The commands of the breakpoint shortcuts (RB-010-16) for the one registry: ids, groups,
//! default bindings (the chords reserved for this job in contract 0.1-w1b) and their MCP tool,
//! `view_set_breakpoint`, which covers every one of them.

use std::sync::{Arc, Weak};

use rb_commands::{
    Binding, Chord, CommandDef, CommandError, CommandId, CommandRegistry, Group, Handler, Key,
    McpCoverage, Modifiers, Shortcut,
};
use serde_json::Value;

use crate::control::Core;

/// The MCP tool that covers every command of the module.
pub const MCP_TOOL: &str = "view_set_breakpoint";
/// Set a view to a breakpoint, a device preset or a free size (arguments); from a shortcut, the
/// free-size input of the shell opens.
pub const SET: &str = "view.set_size";
/// `view.set_breakpoint.<n>`: breakpoint n (1-9) in config order.
pub const SET_PREFIX: &str = "view.set_breakpoint.";
/// The next breakpoint by width.
pub const NEXT: &str = "view.next_breakpoint";
/// The previous breakpoint by width.
pub const PREV: &str = "view.prev_breakpoint";
/// One px wider.
pub const WIDER: &str = "view.nudge_width.plus";
/// One px narrower.
pub const NARROWER: &str = "view.nudge_width.minus";
/// Back to the breakpoint of the view's cell.
pub const RESET: &str = "view.reset_size";
/// `breakpoints.detected.<n>`: the n-th CSS breakpoint the analyser found (RB-010-13).
pub const DETECTED_PREFIX: &str = "breakpoints.detected.";

/// The commands with one leader chord each: id, group, the key after the leader. `Leader ]`,
/// `[`, `.`, `-` and `0` are single keys (no group key, so no group opens); `,` opens the
/// Settings group, hence `-` for one px narrower.
const SINGLE: [(&str, Group, &[char]); 6] = [
    (SET, Group::View, &['V', 'S']),
    (NEXT, Group::Breakpoints, &[']']),
    (PREV, Group::Breakpoints, &['[']),
    (WIDER, Group::Breakpoints, &['.']),
    (NARROWER, Group::Breakpoints, &['-']),
    (RESET, Group::Breakpoints, &['0']),
];

/// The definitions: the single commands, then `view.set_breakpoint.1...9` (`Ctrl+Shift+<n>`
/// and `Leader V <n>`), then `breakpoints.detected.1...9` (`Leader B <n>`). Title key
/// `commands.<id>`.
pub fn definitions() -> Result<Vec<CommandDef>, CommandError> {
    let mut defs = Vec::new();
    for (id, group, keys) in SINGLE {
        defs.push(def(id, group, vec![leader(keys)])?);
    }
    for digit in '1'..='9' {
        let direct = Binding::Direct(Shortcut {
            modifiers: Modifiers::CTRL_SHIFT,
            key: Key::Char(digit),
        });
        defs.push(def(
            &format!("{SET_PREFIX}{digit}"),
            Group::View,
            vec![direct, leader(&['V', digit])],
        )?);
    }
    for digit in '1'..='9' {
        defs.push(def(
            &format!("{DETECTED_PREFIX}{digit}"),
            Group::Breakpoints,
            vec![leader(&['B', digit])],
        )?);
    }
    Ok(defs)
}

/// Whether `id` is a command of this module.
pub fn is_own(id: &str) -> bool {
    SINGLE.iter().any(|(own, _, _)| *own == id)
        || numbered(id, SET_PREFIX).is_some()
        || numbered(id, DETECTED_PREFIX).is_some()
}

/// The number 1-9 after `prefix` (`view.set_breakpoint.3` -> 3).
pub fn numbered(id: &str, prefix: &str) -> Option<usize> {
    let number: usize = id.strip_prefix(prefix)?.parse().ok()?;
    (1..=9).contains(&number).then_some(number)
}

/// Registers every command with `registry`, each running [`super::run`]; returns what the
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

fn leader(keys: &[char]) -> Binding {
    Binding::Leader(Chord {
        keys: keys.iter().map(|key| Key::Char(*key)).collect(),
    })
}

fn def(id: &str, group: Group, default_bindings: Vec<Binding>) -> Result<CommandDef, CommandError> {
    Ok(CommandDef {
        id: CommandId::new(id)?,
        title_key: format!("commands.{id}"),
        group,
        default_bindings,
        mcp: McpCoverage::Covered(MCP_TOOL.to_owned()),
    })
}

#[cfg(test)]
#[path = "defs_tests.rs"]
mod tests;
