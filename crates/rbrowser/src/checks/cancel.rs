//! `checks.cancel` (RB-080-89, `Leader K Q`, the palette, the findings panel's Cancel, the MCP
//! tool `checks_cancel`): stops the check run in progress. Jobs not started yet are skipped, the
//! running ones are dropped; what the finished ones found stays and is shown and written as the
//! run. Without a run in progress it changes nothing.

use std::sync::{Arc, Weak};

use rb_commands::{
    Binding, Chord, CommandDef, CommandError, CommandId, CommandRegistry, Group, Handler, Key,
    McpCoverage,
};
use serde_json::{Value, json};

use super::progress;
use crate::control::Core;

/// The command.
pub const CANCEL_COMMAND: &str = "checks.cancel";
/// Its MCP tool.
pub const CANCEL_TOOL: &str = "checks_cancel";
/// The key after `Leader K`: Q for quit (`K X` is the coverage plugin's stop).
const CANCEL_KEY: char = 'Q';

/// Cancels the run in progress: `{ "cancelled": true }`, or `false` when none is.
pub fn run() -> Value {
    json!({ "cancelled": progress::cancel_current() })
}

/// The definition; title key `commands.checks.cancel`.
///
/// # Errors
///
/// An id the registry would refuse (a test holds it valid).
pub fn definitions() -> Result<Vec<CommandDef>, CommandError> {
    Ok(vec![CommandDef {
        id: CommandId::new(CANCEL_COMMAND)?,
        title_key: format!("commands.{CANCEL_COMMAND}"),
        group: Group::Checks,
        default_bindings: vec![Binding::Leader(Chord {
            keys: vec![Key::Char(Group::Checks.leader_key()), Key::Char(CANCEL_KEY)],
        })],
        mcp: McpCoverage::Covered(CANCEL_TOOL.to_owned()),
    }])
}

/// Registers the command with `registry`; returns what the registry refused.
pub fn register(registry: &mut CommandRegistry, core: &Arc<Core>) -> Vec<CommandError> {
    let defs = match definitions() {
        Ok(defs) => defs,
        Err(error) => return vec![error],
    };
    defs.into_iter()
        .filter_map(|def| registry.register(def, handler(Arc::downgrade(core))).err())
        .collect()
}

fn handler(core: Weak<Core>) -> Handler {
    Arc::new(move |_args: Value| {
        core.upgrade()
            .ok_or_else(|| CommandError::Failed("the window is closed".to_owned()))?;
        Ok(run())
    })
}

#[cfg(test)]
#[path = "cancel_tests.rs"]
mod tests;
