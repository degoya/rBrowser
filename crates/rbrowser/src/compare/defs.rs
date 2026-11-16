//! The commands of the environment comparison for the one registry (RB-080-03), in the
//! environments group: `env.compare.live` (`Leader E C`) shows the active breakpoint once per
//! environment, synced, and leaves it again; `env.compare.run` (`Leader E R`) starts a pixel-diff
//! run in hidden render views; `env.compare.report` (`Leader E O`) opens the latest run's
//! `report.html` in the active view; `env.compare.cancel` (`Leader E X`) cancels the run in
//! progress.

use std::sync::{Arc, Weak};

use rb_commands::{
    Binding, Chord, CommandDef, CommandError, CommandId, CommandRegistry, Group, Handler, Key,
    McpCoverage,
};
use serde_json::Value;

use super::tool::{CANCEL_TOOL, COMPARE_TOOL, LIVE_TOOL, RESULT_TOOL};
use crate::control::Core;

/// Show (or leave) the live comparison.
pub const LIVE: &str = "env.compare.live";
/// Start a diff run.
pub const RUN: &str = "env.compare.run";
/// Open the latest run's report.
pub const REPORT: &str = "env.compare.report";
/// Cancel the run in progress.
pub const CANCEL: &str = "env.compare.cancel";

/// Id, the keys after the leader, MCP tool.
const ROWS: [(&str, [char; 2], &str); 4] = [
    (LIVE, ['E', 'C'], LIVE_TOOL),
    (RUN, ['E', 'R'], COMPARE_TOOL),
    (REPORT, ['E', 'O'], RESULT_TOOL),
    (CANCEL, ['E', 'X'], CANCEL_TOOL),
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

/// Registers the commands with `registry`, each running [`super::app::run`]; returns what the
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
        super::app::run(&core, &id, &args)
    })
}

#[cfg(test)]
#[path = "defs_tests.rs"]
mod tests;
