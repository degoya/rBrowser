//! The command of copying cookies for the one registry (RB-040-05): `cookies.copy`, `Leader E J`
//! (the cookie jar), MCP `cookies_copy`. It sits in the Environments group beside the sessions
//! it follows (`Leader E S`, `Leader E K`): the job's `Leader S C` is `sync.toggle_scroll`, and
//! `Leader E C` is planned for the environment comparison (RB-080-03).

use std::sync::{Arc, Weak};

use rb_commands::{
    Binding, Chord, CommandDef, CommandError, CommandId, CommandRegistry, Group, Handler, Key,
    McpCoverage,
};
use serde_json::Value;

use crate::control::Core;

/// Copy the cookies of the Chromium sessions into the Firefox and WebKit views
/// (`{ source?, target?, domains? }`, `super::CopyArgs`).
pub const COPY: &str = "cookies.copy";
/// The MCP tool of `cookies.copy`.
pub const COPY_TOOL: &str = "cookies_copy";

/// The definition; title key `commands.cookies.copy`.
pub fn definitions() -> Result<Vec<CommandDef>, CommandError> {
    Ok(vec![CommandDef {
        id: CommandId::new(COPY)?,
        title_key: format!("commands.{COPY}"),
        group: Group::Environments,
        default_bindings: vec![Binding::Leader(Chord {
            keys: vec![Key::Char('E'), Key::Char('J')],
        })],
        mcp: McpCoverage::Covered(COPY_TOOL.to_owned()),
    }])
}

/// Registers `cookies.copy` with `registry`, running [`super::run`]; returns what the registry
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
