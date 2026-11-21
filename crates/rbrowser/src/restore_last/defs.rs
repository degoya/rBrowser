//! The command of the last pages for the one registry (RB-092-04): `views.restore_last`
//! (`Leader R L`, the Reload group: it loads the views again), MCP `views_restore_last`.

use std::sync::{Arc, Weak};

use rb_commands::{
    Binding, Chord, CommandDef, CommandError, CommandId, CommandRegistry, Group, Handler, Key,
    McpCoverage,
};
use serde_json::Value;

use crate::control::Core;

/// Open the pages of the last session now (`{ target?: "all" | viewId }`, every view when
/// absent).
pub const RESTORE_LAST: &str = "views.restore_last";
/// Its MCP tool.
pub const TOOL: &str = "views_restore_last";

/// The definition; title key `commands.views.restore_last`.
pub fn definitions() -> Result<Vec<CommandDef>, CommandError> {
    Ok(vec![CommandDef {
        id: CommandId::new(RESTORE_LAST)?,
        title_key: format!("commands.{RESTORE_LAST}"),
        group: Group::Reload,
        default_bindings: vec![Binding::Leader(Chord {
            keys: vec![Key::Char('R'), Key::Char('L')],
        })],
        mcp: McpCoverage::Covered(TOOL.to_owned()),
    }])
}

/// Registers the command with `registry`, running [`super::run`]; returns what the registry
/// refused (a test holds it empty).
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
        super::run(&core, &args)
    })
}

#[cfg(test)]
#[path = "defs_tests.rs"]
mod tests;
