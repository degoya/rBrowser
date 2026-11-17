//! The commands of the layout modes for the one registry (RB-080-04): ids, default leader
//! chords in the Layout group (`L`, beside `L 1`...`9`, `L F`, `L L`, `L Z`, `L [`, `L ]`) and
//! MCP coverage. `layout.mode.grid` (`L G`) leads back to the grid; the divider moves with
//! `L Left` / `L Right`, the canvas centres with `L .`; `L N` saves the layout as a new preset.
//! The canvas zooms with `L 0` (100 %), `L Plus` and `L -`, RB-080-94; every view in sight is
//! `layout.zoom_to_fit` (`L Z`, RB-090-63).

use std::sync::{Arc, Weak};

use rb_commands::McpCoverage;
use rb_commands::{Binding, CommandDef, CommandError, CommandId, CommandRegistry, Group, Handler};
use serde_json::Value;

use crate::control::Core;

/// Back to the grid of the current preset.
pub const MODE_GRID: &str = "layout.mode.grid";
/// Horizontal (`{ height?, gap? }`).
pub const MODE_HORIZONTAL: &str = "layout.mode.horizontal";
/// Split (`{ orientation?, ratio?, primary?, secondary? }`).
pub const MODE_SPLIT: &str = "layout.mode.split";
/// Canvas (`{ snap? }`).
pub const MODE_CANVAS: &str = "layout.mode.canvas";
/// The minimap on or off for the session.
pub const MINIMAP_TOGGLE: &str = "layout.minimap.toggle";
/// The split's sides swapped.
pub const SPLIT_SWAP: &str = "layout.split.swap";
/// The divider one step towards the secondary side.
pub const SPLIT_GROW: &str = "layout.split.grow";
/// The divider one step towards the primary side.
pub const SPLIT_SHRINK: &str = "layout.split.shrink";
/// Canvas: centre on `{ x, y }` (a minimap click), on `{ view }` (the view menu, RB-080-41) or
/// on the active view; horizontal: scroll to `{ index }` or the active view.
pub const CENTER: &str = "layout.center";
/// Canvas: place a view (`{ view?, x?, y?, zoom? }`); without `x`/`y` in the middle of what the
/// area shows.
pub const CANVAS_PLACE: &str = "layout.canvas.place";
/// The layout in effect saved as the preset `{ id, file? }`; without `id` the shell asks for the
/// name (`uiCommand`).
pub const SAVE_PRESET: &str = "layout.save_preset";

/// Canvas: the active page at 100 %, or at `{ level }` (RB-080-94).
pub const CANVAS_ZOOM_RESET: &str = super::canvas_level::RESET;
/// Canvas: one zoom step up.
pub const CANVAS_ZOOM_IN: &str = super::canvas_level::ZOOM_IN;
/// Canvas: one zoom step down.
pub const CANVAS_ZOOM_OUT: &str = super::canvas_level::ZOOM_OUT;

/// Why the minimap has no MCP tool (job file: "reine Anzeige").
pub const MINIMAP_OMITTED: &str = "display only: the minimap in the toolbar";
/// Why centring has no MCP tool.
pub const CENTER_OMITTED: &str =
    "ui-only: what the area shows of the canvas or strip; canvas_place moves views";

/// Why a step of the canvas zoom has no MCP tool.
pub const ZOOM_STEP_OMITTED: &str =
    "ui-only: one step of the canvas zoom; canvas_zoom sets any level";

/// Id, default binding, MCP tool (`Ok`) or why there is none (`Err`).
const ROWS: [(&str, &str, Result<&str, &str>); 14] = [
    (MODE_GRID, "Ctrl+Shift+Space L G", Ok("layout_set")),
    (MODE_HORIZONTAL, "Ctrl+Shift+Space L H", Ok("layout_set")),
    (MODE_SPLIT, "Ctrl+Shift+Space L S", Ok("layout_set")),
    (MODE_CANVAS, "Ctrl+Shift+Space L C", Ok("layout_set")),
    (MINIMAP_TOGGLE, "Ctrl+Shift+Space L M", Err(MINIMAP_OMITTED)),
    (SPLIT_SWAP, "Ctrl+Shift+Space L W", Ok("layout_set")),
    (SPLIT_GROW, "Ctrl+Shift+Space L Right", Ok("layout_set")),
    (SPLIT_SHRINK, "Ctrl+Shift+Space L Left", Ok("layout_set")),
    (CENTER, "Ctrl+Shift+Space L .", Err(CENTER_OMITTED)),
    (CANVAS_PLACE, "Ctrl+Shift+Space L P", Ok("canvas_place")),
    (
        SAVE_PRESET,
        "Ctrl+Shift+Space L N",
        Ok("layout_save_preset"),
    ),
    // The canvas zoom (RB-080-94); `Ctrl+0` on the canvas board is the reservation list's
    // context entry for `canvas.zoom.reset`.
    (CANVAS_ZOOM_RESET, "Ctrl+Shift+Space L 0", Ok("canvas_zoom")),
    (
        CANVAS_ZOOM_IN,
        "Ctrl+Shift+Space L Plus",
        Err(ZOOM_STEP_OMITTED),
    ),
    (
        CANVAS_ZOOM_OUT,
        "Ctrl+Shift+Space L -",
        Err(ZOOM_STEP_OMITTED),
    ),
];

/// The definitions; title key `commands.<id>`.
pub fn definitions() -> Result<Vec<CommandDef>, CommandError> {
    ROWS.iter()
        .map(|(id, binding, mcp)| {
            Ok(CommandDef {
                id: CommandId::new(id)?,
                title_key: format!("commands.{id}"),
                group: Group::Layout,
                default_bindings: vec![binding.parse::<Binding>()?],
                mcp: match mcp {
                    Ok(tool) => McpCoverage::Covered((*tool).to_owned()),
                    Err(reason) => McpCoverage::Omitted((*reason).to_owned()),
                },
            })
        })
        .collect()
}

/// Whether `id` is a command of this module.
pub fn is_own(id: &str) -> bool {
    ROWS.iter().any(|(own, _, _)| *own == id)
}

/// Registers the commands with `registry`, each running [`super::actions::run`]; returns what
/// the registry refused (a test holds it empty).
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
        super::actions::run(&core, &id, &args)
    })
}

#[cfg(test)]
#[path = "defs_tests.rs"]
mod tests;
