//! The commands of sitemap capture for the one registry (RB-050-05): ids, groups, default leader
//! chords and MCP coverage. `sitemap.capture` (`Leader C M`) starts a run with the call's
//! arguments (the project's start URL and every breakpoint without), `sitemap.cancel`
//! (`Leader C X`) cancels the one in progress, `sitemap.gallery` (`Leader C I`, RB-090-83) opens
//! a run's gallery in its own window, `panels.sitemap` (`Leader P G`) shows the run in the
//! toolbar strip and answers its status.

use std::sync::{Arc, Weak};

use rb_commands::{
    Binding, Chord, CommandDef, CommandError, CommandId, CommandRegistry, Group, Handler, Key,
    McpCoverage,
};
use serde_json::Value;

use super::tool::{CANCEL_TOOL, CAPTURE_TOOL, GALLERY_TOOL, STATUS_TOOL};
use crate::control::Core;

/// Start (or resume) a capture.
pub const CAPTURE: &str = "sitemap.capture";
/// Cancel the capture in progress.
pub const CANCEL: &str = "sitemap.cancel";
/// Open a run's gallery (RB-090-83).
pub const GALLERY: &str = "sitemap.gallery";
/// Show the capture in the toolbar strip.
pub const PANEL: &str = "panels.sitemap";

/// Id, group, the keys after the leader, MCP tool.
const ROWS: [(&str, Group, [char; 2], &str); 4] = [
    (CAPTURE, Group::Capture, ['C', 'M'], CAPTURE_TOOL),
    (CANCEL, Group::Capture, ['C', 'X'], CANCEL_TOOL),
    // I for images: G is `visual.lfs_setup`'s.
    (GALLERY, Group::Capture, ['C', 'I'], GALLERY_TOOL),
    (PANEL, Group::Panels, ['P', 'G'], STATUS_TOOL),
];

/// The definitions; title key `commands.<id>`.
pub fn definitions() -> Result<Vec<CommandDef>, CommandError> {
    ROWS.iter()
        .map(|(id, group, keys, tool)| {
            Ok(CommandDef {
                id: CommandId::new(id)?,
                title_key: format!("commands.{id}"),
                group: *group,
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
