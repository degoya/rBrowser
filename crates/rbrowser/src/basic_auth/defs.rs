//! The command of Basic auth for the one registry (RB-094-08): `environment.auth`, `Leader E A`
//! in the Environments group, a person's dialog that MCP never runs.

use std::sync::{Arc, Weak};

use rb_commands::{
    Binding, Chord, CommandDef, CommandError, CommandId, CommandRegistry, Group, Handler, Key,
    McpCoverage,
};
use serde_json::Value;

use crate::control::Core;

/// Set, change or remove the Basic auth credential of an environment.
pub const AUTH: &str = "environment.auth";
/// Why MCP has no tool for it (the coverage table).
pub const SECRETS_OMITTED: &str =
    "secrets: Basic auth credentials and login state are a person's, never read or set over MCP";

/// The definition; title key `commands.environment.auth`.
pub fn definitions() -> Result<Vec<CommandDef>, CommandError> {
    Ok(vec![CommandDef {
        id: CommandId::new(AUTH)?,
        title_key: format!("commands.{AUTH}"),
        group: Group::Environments,
        default_bindings: vec![Binding::Leader(Chord {
            keys: vec![Key::Char('E'), Key::Char('A')],
        })],
        mcp: McpCoverage::Omitted(SECRETS_OMITTED.to_owned()),
    }])
}

/// Refuses `environment.auth` to an MCP client (the command has no tool; this holds even if a
/// generic tool ever named it).
pub fn refuse_mcp(id: &str) -> Result<(), CommandError> {
    if id == AUTH {
        return Err(CommandError::Failed(format!("{AUTH}: {SECRETS_OMITTED}")));
    }
    Ok(())
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
    Arc::new(move |args: Value| {
        let core = core
            .upgrade()
            .ok_or_else(|| CommandError::Failed("the window is closed".to_owned()))?;
        super::command::run(&core, &args)
    })
}

#[cfg(test)]
#[path = "defs_tests.rs"]
mod tests;
