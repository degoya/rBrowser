//! The commands of project bookmarks for the one registry (RB-070-06): ids, group, default
//! leader chords and MCP coverage. They sit in the Panels group (`P`): the job's `Leader V B` is
//! `view.back` and `Leader V L` an emulation extra (RB-080-10). MCP opens a bookmark with
//! `navigate` (the URL from `bookmarks_list`) and adds one with `config_set`;
//! `bookmarks.make_relative` (RB-080-48) is the tool `bookmarks_make_relative`.

use std::sync::{Arc, Weak};

use rb_commands::{
    Binding, Chord, CommandDef, CommandError, CommandId, CommandRegistry, Group, Handler, Key,
    McpCoverage,
};
use serde_json::Value;

use crate::control::Core;

/// Open a bookmark (`{ id, view? }`); without an id the shell's quick pick opens.
pub const OPEN: &str = "bookmarks.open";
/// Bookmark the current page (`{ label?, path? | url?, group?, shortcut?, scope?, id? }`);
/// without a label, path or URL the shell's dialog opens.
pub const ADD: &str = "bookmarks.add";
/// Turn every absolute bookmark that lies under an environment of the project into a path that
/// follows the environment (RB-080-48); no arguments.
pub const MAKE_RELATIVE: &str = "bookmarks.make_relative";

/// Id, the keys after the leader, MCP tool.
const ROWS: [(&str, [char; 2], &str); 3] = [
    (OPEN, ['P', 'B'], "navigate"),
    (ADD, ['P', 'K'], "config_set"),
    (MAKE_RELATIVE, ['P', 'P'], "bookmarks_make_relative"),
];

/// The definitions; title key `commands.<id>`.
pub fn definitions() -> Result<Vec<CommandDef>, CommandError> {
    ROWS.iter()
        .map(|(id, keys, tool)| {
            Ok(CommandDef {
                id: CommandId::new(id)?,
                title_key: format!("commands.{id}"),
                group: Group::Panels,
                default_bindings: vec![Binding::Leader(Chord {
                    keys: keys.iter().map(|key| Key::Char(*key)).collect(),
                })],
                mcp: McpCoverage::Covered((*tool).to_owned()),
            })
        })
        .collect()
}

/// Whether `id` is a command of this module.
pub fn is_own(id: &str) -> bool {
    ROWS.iter().any(|(own, _, _)| *own == id)
}

/// Registers the commands with `registry`, each running [`super::run`]; returns what the
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

#[cfg(test)]
#[path = "defs_tests.rs"]
mod tests;
