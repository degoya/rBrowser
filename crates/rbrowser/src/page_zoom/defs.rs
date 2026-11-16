//! The commands of the page zoom (RB-090-20) in the View group (`V`): `view.page_zoom.reset`
//! (`Leader V Home`; with the focus in a page also Ctrl+0), `view.page_zoom.in` (`Leader V Up`,
//! Ctrl+Plus) and `view.page_zoom.out` (`Leader V Down`, Ctrl+Minus). The browser's keys are no
//! bindings of the registry: the reservation list keeps them for the page's browser, and the
//! views' keyboard handler gives them this meaning (`cef_app/keys.rs`). MCP `view_page_zoom`
//! runs all three. The ids are not `view.zoom_*`: those zoom the grid's cell.

use std::sync::{Arc, Weak};

use rb_commands::{
    Binding, Chord, CommandDef, CommandError, CommandId, CommandRegistry, Group, Handler, Key,
    McpCoverage,
};
use serde_json::Value;

use crate::control::Core;

/// Back to 100 %, or `{ level }` as a factor.
pub const RESET: &str = "view.page_zoom.reset";
/// One of Chromium's steps larger.
pub const ZOOM_IN: &str = "view.page_zoom.in";
/// One of Chromium's steps smaller.
pub const ZOOM_OUT: &str = "view.page_zoom.out";
/// The MCP tool of all three.
pub const TOOL: &str = "view_page_zoom";

/// Id, the key after `V`.
const ROWS: [(&str, Key); 3] = [
    (RESET, Key::Home),
    (ZOOM_IN, Key::Up),
    (ZOOM_OUT, Key::Down),
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
                group: Group::View,
                default_bindings: vec![Binding::Leader(Chord {
                    keys: vec![Key::Char(Group::View.leader_key()), *key],
                })],
                mcp: McpCoverage::Covered(TOOL.to_owned()),
            })
        })
        .collect()
}

/// Registers the three commands with `registry`; returns what the registry refused.
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
