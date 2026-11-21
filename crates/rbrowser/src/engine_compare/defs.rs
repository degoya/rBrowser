//! The command of the engine comparison for the one registry (RB-094-03), in the capture group:
//! `compare.engines` (`Leader C Q`) compares the target view's page at its breakpoint in
//! Chromium, Firefox and WebKit and opens the report when done.

use std::sync::{Arc, Weak};

use rb_commands::{
    Binding, Chord, CommandDef, CommandError, CommandId, CommandRegistry, Group, Handler, Key,
    McpCoverage,
};
use serde_json::Value;

use super::tool::COMPARE_TOOL;
use crate::control::Core;

/// Compare the engines.
pub const ENGINES_COMMAND: &str = "compare.engines";

/// The definition; title key `commands.compare.engines`.
pub fn definitions() -> Result<Vec<CommandDef>, CommandError> {
    Ok(vec![CommandDef {
        id: CommandId::new(ENGINES_COMMAND)?,
        title_key: format!("commands.{ENGINES_COMMAND}"),
        group: Group::Capture,
        default_bindings: vec![Binding::Leader(Chord {
            keys: vec![Key::Char('C'), Key::Char('Q')],
        })],
        mcp: McpCoverage::Covered(COMPARE_TOOL.to_owned()),
    }])
}

/// Registers the command with `registry`, running [`super::app::run`]; returns what the
/// registry refused (a test holds it empty).
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
    Arc::new(move |args: Value| {
        let core = core
            .upgrade()
            .ok_or_else(|| CommandError::Failed("the window is closed".to_owned()))?;
        super::app::run(&core, &args)
    })
}

#[cfg(test)]
#[path = "defs_tests.rs"]
mod tests;
