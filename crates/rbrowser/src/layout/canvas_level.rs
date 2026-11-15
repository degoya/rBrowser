//! The canvas's zoom as the page's zoom (RB-080-94): the level is the active view's camera zoom
//! times its own item zoom - 100 % shows its page at its real size. The commands
//! `canvas.zoom.reset` (100 %, or `{ level }`), `canvas.zoom.in` and `canvas.zoom.out` (one of
//! [`STEPS`]) and `canvas.zoom.fit` (every view, as `layout.zoom_to_fit`); MCP `canvas_zoom`.
//! Without `x`/`y` (DIP from the board's top-left corner) the active view stays in the middle
//! of the area; with them the canvas point under that place stays under it (Ctrl+wheel's
//! rule). The level goes up to [`MAX_PAGE_ZOOM`] for the largest view
//! (`canvas::camera_limit`). On the canvas board `Ctrl+0` runs `canvas.zoom.reset`: the
//! context entry of the reservation list (`crates/rb-commands/data/reserved-shortcuts.json`).

use rb_commands::CommandError;
use rb_config::{CanvasItem, LayoutMode};
use serde::Deserialize;
use serde_json::{Value, json};

use super::Item;
use super::actions::not_in;
use super::canvas::{self, Camera, MAX_PAGE_ZOOM};
use super::canvas_board::checked;
use super::canvas_ops::{active_index, canvas_state, fit_canvas, mode_and_area, set_camera};
use super::canvas_zoom::to_zoom_at;
use crate::control::Core;
use crate::host::{Host, LABEL_HEIGHT};

/// 100 %, the page at its real size.
pub const RESET: &str = "canvas.zoom.reset";
/// One step of [`STEPS`] up.
pub const ZOOM_IN: &str = "canvas.zoom.in";
/// One step of [`STEPS`] down.
pub const ZOOM_OUT: &str = "canvas.zoom.out";
/// Every view in sight.
pub const FIT: &str = "canvas.zoom.fit";

/// The levels `canvas.zoom.in` and `canvas.zoom.out` step through, down to [`MIN_LEVEL`]; the
/// status bar's presets are among them.
pub const STEPS: [f64; 11] = [0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 0.75, 1.0, 1.25, 1.5, 2.0];
/// The lowest level a view reaches: the camera's lowest zoom on the smallest item zoom
/// (0.05 x 0.1, 0.5 %); the status bar's slider starts there (`ui/src/statusbar/canvasZoom.ts`).
pub const MIN_LEVEL: f64 = super::canvas::ZOOM_RANGE.0 * rb_config::CANVAS_ZOOM_RANGE.0;

/// The arguments of the zoom commands.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct ZoomArgs {
    /// `canvas.zoom.reset`: the level, 1 for 100 %; 1 when absent.
    level: Option<f64>,
    /// The place that stays where it is, in DIP from the board's top-left corner.
    x: Option<i32>,
    y: Option<i32>,
}

/// The level `step` steps from `level`: the next of [`STEPS`] above (`up`) or below. Past the
/// last step in its direction the level stays - a step never goes the other way (a canvas
/// fitted below 1 % zoomed in on "zoom out").
pub fn step(level: f64, up: bool) -> f64 {
    // Relative, so the small steps count as well as the large ones.
    const SLACK: f64 = 0.02;
    let next = if up {
        STEPS
            .iter()
            .copied()
            .find(|step| *step > level * (1.0 + SLACK))
    } else {
        STEPS
            .iter()
            .rev()
            .copied()
            .find(|step| *step < level * (1.0 - SLACK))
    };
    next.unwrap_or(level)
}

/// The page's zoom of a view at `item` zoom through `camera`.
pub fn level_of(camera: Camera, item: f64) -> f64 {
    camera.clamped().zoom * item
}

/// The camera zoom that shows a view at `item` zoom at `level`.
pub fn camera_zoom_for(level: f64, item: f64) -> f64 {
    level / item.max(rb_config::CANVAS_ZOOM_RANGE.0)
}

/// The item of the active view among `items` (the first without one).
fn active_item<'a>(host: &Host, items: &'a [CanvasItem]) -> Option<(usize, &'a CanvasItem)> {
    let index = active_index(host).filter(|index| *index < items.len());
    let index = index.unwrap_or(0);
    items.get(index).map(|item| (index, item))
}

/// The level of the canvas in effect through `camera`: the active view's page zoom; `None`
/// without a view.
pub fn level_at(host: &Host, items: &[CanvasItem], camera: Camera) -> Option<f64> {
    active_item(host, items).map(|(_, item)| level_of(camera, item.zoom))
}

/// Runs the zoom command `id`; answers the level it set.
pub fn run(core: &Core, id: &str, args: &Value) -> Result<Value, CommandError> {
    let args: ZoomArgs = if args.is_null() {
        ZoomArgs::default()
    } else {
        serde_json::from_value(args.clone())
            .map_err(|error| CommandError::InvalidArgs(error.to_string()))?
    };
    let at = match (args.x, args.y) {
        (Some(x), Some(y)) => {
            checked(x, y)?;
            Some((x, y))
        }
        (None, None) => None,
        _ => return Err(CommandError::InvalidArgs("pass x and y, or neither".into())),
    };
    let host = core.host();
    let (spec, area) = mode_and_area(host, LayoutMode::Canvas)?;
    if id == FIT {
        if !fit_canvas(core) {
            return Err(not_in(LayoutMode::Canvas));
        }
        return Ok(json!({ "level": current(host) }));
    }
    let sizes = host.mode_sizes();
    let (items, camera) = canvas_state(&spec, &sizes, area);
    let Some((index, item)) = active_item(host, &items) else {
        return Err(CommandError::Failed("there is no view".to_owned()));
    };
    let level = match id {
        RESET => match args.level {
            Some(level) if level.is_finite() && level > 0.0 => level,
            Some(_) => return Err(CommandError::InvalidArgs("level is no zoom".into())),
            None => 1.0,
        },
        ZOOM_IN | ZOOM_OUT => step(level_of(camera, item.zoom), id == ZOOM_IN),
        other => {
            return Err(CommandError::InvalidArgs(format!(
                "unknown command {other}"
            )));
        }
    };
    let zoom = camera_zoom_for(level.clamp(MIN_LEVEL, MAX_PAGE_ZOOM), item.zoom)
        .min(canvas::camera_limit(&items));
    let next = match at {
        Some(at) => to_zoom_at(camera, at, zoom),
        None => centred(&items, &sizes, index, area, zoom),
    };
    set_camera(core, next);
    Ok(json!({ "level": current(host) }))
}

/// The camera at `zoom` with the view `index` in the middle of `area`.
fn centred(
    items: &[CanvasItem],
    sizes: &[Item],
    index: usize,
    area: rb_engine::CellRect,
    zoom: f64,
) -> Camera {
    let rect = items
        .get(index)
        .zip(sizes.get(index))
        .map(|(item, size)| canvas::outer(item, size, LABEL_HEIGHT))
        .unwrap_or_default();
    canvas::center_on(
        rect.x + rect.width / 2.0,
        rect.y + rect.height / 2.0,
        area,
        zoom,
    )
}

/// The level of the canvas in effect, rounded to a tenth of a per cent; `None` outside it.
pub fn current(host: &Host) -> Option<f64> {
    let (spec, area) = mode_and_area(host, LayoutMode::Canvas).ok()?;
    let (items, camera) = canvas_state(&spec, &host.mode_sizes(), area);
    level_at(host, &items, camera).map(|level| (level * 1000.0).round() / 1000.0)
}

#[cfg(test)]
#[path = "canvas_level_tests.rs"]
mod tests;
