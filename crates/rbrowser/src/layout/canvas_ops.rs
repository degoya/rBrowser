//! The canvas's and the strip's commands (RB-080-04): centre the canvas on a point (a minimap
//! click) or the active view, scroll the strip, place a view (`canvas_place`) with snapping,
//! and zoom the canvas to fit.

use rb_commands::CommandError;
use rb_config::{CANVAS_ZOOM_RANGE, CanvasItem, LayoutMode, MAX_CANVAS_COORD};
use rb_engine::{CellRect, ViewHost, ViewId};
use serde::Deserialize;
use serde_json::{Value, json};

use super::actions::{active_breakpoint, not_in, persist_session};
use super::canvas::{self, Camera, WorldRect};
use super::horizontal::{self, Strip};
use super::spec::{ModeKind, ModeSpec};
use super::{Item, contains};
use crate::control::Core;
use crate::host::{Host, LABEL_HEIGHT};

/// `layout.center`'s arguments.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct CenterArgs {
    /// Canvas: the point to centre on, in canvas DIP.
    x: Option<f64>,
    y: Option<f64>,
    /// Horizontal: the view to scroll to, by its place in the strip.
    index: Option<usize>,
    /// Canvas: the view (id) to centre on instead of the active one (RB-080-41).
    view: Option<ViewId>,
}

/// `layout.canvas.place`'s arguments.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct PlaceArgs {
    /// A view id (number) or breakpoint id; the active view when absent.
    view: Option<Value>,
    x: Option<f64>,
    y: Option<f64>,
    zoom: Option<f64>,
}

fn parse<T: for<'de> Deserialize<'de> + Default>(args: &Value) -> Result<T, CommandError> {
    if args.is_null() {
        return Ok(T::default());
    }
    serde_json::from_value(args.clone())
        .map_err(|error| CommandError::InvalidArgs(error.to_string()))
}

/// The spec in effect and the area; the error names the mode a command needs.
pub(super) fn mode_and_area(
    host: &Host,
    mode: LayoutMode,
) -> Result<(ModeSpec, CellRect), CommandError> {
    let spec = host
        .mode_spec()
        .filter(|spec| spec.mode() == mode)
        .ok_or_else(|| not_in(mode))?;
    let area = host
        .grid_area()
        .ok_or_else(|| CommandError::Failed("the grid area is not laid out yet".to_owned()))?;
    Ok((spec, area))
}

/// `layout.center`: the canvas centred on `{ x, y }` or on the active view; in horizontal mode
/// the strip scrolled to `{ index }` or the active view.
pub fn center(core: &Core, args: &Value) -> Result<Value, CommandError> {
    let args: CenterArgs = parse(args)?;
    let host = core.host();
    match host.mode_spec().map(|spec| spec.mode()) {
        Some(LayoutMode::Horizontal) => {
            let index = match args.index {
                Some(index) => index,
                None => active_index(host).unwrap_or(0),
            };
            scroll_to(core, index)
        }
        Some(LayoutMode::Canvas) => {
            let (spec, area) = mode_and_area(host, LayoutMode::Canvas)?;
            let sizes = host.mode_sizes();
            let (items, camera) = canvas_state(&spec, &sizes, area);
            let (x, y) = match (args.x, args.y) {
                (Some(x), Some(y)) if x.is_finite() && y.is_finite() => (x, y),
                (None, None) => {
                    let index = match args.view {
                        Some(view) => canvas_index(
                            host,
                            &spec,
                            Some(&json!(view.0)),
                            items.len().min(sizes.len()),
                        )?,
                        None => active_index(host).unwrap_or(0),
                    };
                    let rect = items
                        .get(index)
                        .zip(sizes.get(index))
                        .map(|(item, size)| canvas::outer(item, size, LABEL_HEIGHT))
                        .unwrap_or_default();
                    (rect.x + rect.width / 2.0, rect.y + rect.height / 2.0)
                }
                _ => return Err(CommandError::InvalidArgs("pass x and y, or neither".into())),
            };
            set_camera(core, canvas::center_on(x, y, area, camera.zoom));
            Ok(json!({ "x": x, "y": y }))
        }
        _ => Err(not_in(LayoutMode::Canvas)),
    }
}

/// `layout.focus_next` / `focus_prev` in horizontal mode: the strip one view on or back.
pub fn scroll(core: &Core, forward: bool) -> Result<Value, CommandError> {
    let host = core.host();
    let (spec, area) = mode_and_area(host, LayoutMode::Horizontal)?;
    let sizes = host.mode_sizes();
    let Some(strip) = strip_of(&spec) else {
        return Err(not_in(LayoutMode::Horizontal));
    };
    let first = horizontal::step(&sizes, area, &strip, forward);
    set_first(core, first);
    Ok(json!({ "first": first }))
}

fn scroll_to(core: &Core, index: usize) -> Result<Value, CommandError> {
    let host = core.host();
    let (spec, area) = mode_and_area(host, LayoutMode::Horizontal)?;
    let sizes = host.mode_sizes();
    let Some(strip) = strip_of(&spec) else {
        return Err(not_in(LayoutMode::Horizontal));
    };
    let first = horizontal::reveal(&sizes, area, &strip, index);
    set_first(core, first);
    Ok(json!({ "first": first }))
}

/// The strip's parameters of `spec`.
pub fn strip_of(spec: &ModeSpec) -> Option<Strip> {
    match spec.kind {
        ModeKind::Horizontal {
            height,
            zoom,
            first,
        } => Some(Strip {
            height,
            gap: spec.gap,
            label: LABEL_HEIGHT,
            zoom,
            first,
        }),
        _ => None,
    }
}

fn set_first(core: &Core, first: usize) {
    let host = core.host();
    host.update_mode(|spec| {
        if let ModeKind::Horizontal { first: current, .. } = &mut spec.kind {
            *current = first;
        }
    });
    host.update_shell_state(|shell| shell.modes.first = first);
}

/// The canvas's items for the views and the camera in effect (fitted when none was set).
pub fn canvas_state(spec: &ModeSpec, sizes: &[Item], area: CellRect) -> (Vec<CanvasItem>, Camera) {
    let ModeKind::Canvas { items, camera, .. } = &spec.kind else {
        return (Vec::new(), Camera::default());
    };
    let items = spec.canvas_items(items, sizes, LABEL_HEIGHT);
    let camera = camera.unwrap_or_else(|| spec.fit(&items, sizes, area, LABEL_HEIGHT));
    (items, camera)
}

pub(super) fn set_camera(core: &Core, camera: Camera) {
    let host = core.host();
    let camera = camera.within(camera_limit(host));
    host.update_mode(|spec| {
        if let ModeKind::Canvas {
            camera: current, ..
        } = &mut spec.kind
        {
            *current = Some(camera);
        }
    });
    host.update_shell_state(|shell| shell.modes.camera = Some(camera));
}

/// The largest camera zoom of the canvas in effect: its largest view at 200 % of its page
/// ([`canvas::camera_limit`], RB-080-94); the camera's range outside the canvas.
pub(super) fn camera_limit(host: &Host) -> f64 {
    let Ok((spec, area)) = mode_and_area(host, LayoutMode::Canvas) else {
        return canvas::ZOOM_RANGE.1;
    };
    let (items, _) = canvas_state(&spec, &host.mode_sizes(), area);
    canvas::camera_limit(&items)
}

/// The user settings changed: a canvas already shown follows `layoutModes.showPartialViews`
/// at once, its views placed again (RB-080-55).
pub fn on_user_settings_changed(core: &Core) {
    let wanted = core.user_settings().layout_modes.show_partial_views;
    let host = core.host();
    let stale = host.mode_spec().is_some_and(
        |spec| matches!(spec.kind, ModeKind::Canvas { partial, .. } if partial != wanted),
    );
    if stale {
        host.update_mode(|spec| {
            if let ModeKind::Canvas { partial, .. } = &mut spec.kind {
                *partial = wanted;
            }
        });
    }
}

/// `layout.zoom_to_fit` in canvas mode: the camera fits every view again. `false` in another
/// mode.
pub fn fit_canvas(core: &Core) -> bool {
    let host = core.host();
    if !host
        .mode_spec()
        .is_some_and(|spec| spec.mode() == LayoutMode::Canvas)
    {
        return false;
    }
    host.update_mode(|spec| {
        if let ModeKind::Canvas { camera, .. } = &mut spec.kind {
            *camera = None;
        }
    });
    host.update_shell_state(|shell| shell.modes.camera = None);
    true
}

/// `layout.canvas.place` (MCP `canvas_place`): a view moved to `x`/`y` (snapped to the grid and
/// to its neighbours' edges) and zoomed to `zoom`; without `x`/`y` into the middle of what the
/// area shows. The items are written into the preset.
pub fn place(core: &Core, args: &Value) -> Result<Value, CommandError> {
    let args: PlaceArgs = parse(args)?;
    let host = core.host();
    let (spec, area) = mode_and_area(host, LayoutMode::Canvas)?;
    let sizes = host.mode_sizes();
    let (items, camera) = canvas_state(&spec, &sizes, area);
    let index = canvas_index(
        host,
        &spec,
        args.view.as_ref(),
        items.len().min(sizes.len()),
    )?;
    let zoom = match args.zoom {
        Some(zoom) if zoom.is_finite() => zoom.clamp(CANVAS_ZOOM_RANGE.0, CANVAS_ZOOM_RANGE.1),
        Some(_) => return Err(CommandError::InvalidArgs("zoom is no number".into())),
        None => items[index].zoom,
    };
    let size = &sizes[index];
    let (width, height) = (f64::from(size.width) * zoom, f64::from(size.height) * zoom);
    let (x, y) = match (args.x, args.y) {
        (Some(x), Some(y)) if x.is_finite() && y.is_finite() => (x, y),
        (None, None) => {
            let shown = canvas::viewport(area, camera);
            (
                shown.x + (shown.width - width) / 2.0,
                shown.y + (shown.height - height) / 2.0,
            )
        }
        _ => return Err(CommandError::InvalidArgs("pass x and y, or neither".into())),
    };
    put(
        core,
        Placement {
            index,
            x,
            y,
            zoom,
            snap: true,
        },
    )
}

/// `view.add { x, y }` on the canvas (RB-080-41): the view of `breakpoint`, just opened, moved
/// to the canvas point `x`/`y` (snapped) and written into the preset.
pub fn place_breakpoint(
    core: &Core,
    breakpoint: &str,
    x: f64,
    y: f64,
) -> Result<Value, CommandError> {
    let host = core.host();
    let (spec, area) = mode_and_area(host, LayoutMode::Canvas)?;
    let sizes = host.mode_sizes();
    let (items, _) = canvas_state(&spec, &sizes, area);
    let index = canvas_index(
        host,
        &spec,
        Some(&json!(breakpoint)),
        items.len().min(sizes.len()),
    )?;
    let zoom = items[index].zoom;
    put(
        core,
        Placement {
            index,
            x,
            y,
            zoom,
            snap: true,
        },
    )
}

/// Where [`put`] moves the item `index` of the canvas: `x`/`y` in canvas DIP, snapped to the
/// grid and the neighbours' edges when `snap` (else only held to whole DIP).
#[derive(Clone, Copy, Debug)]
pub(super) struct Placement {
    pub(super) index: usize,
    pub(super) x: f64,
    pub(super) y: f64,
    pub(super) zoom: f64,
    pub(super) snap: bool,
}

/// Moves one item of the canvas in effect to `placement` (never past [`MAX_CANVAS_COORD`]),
/// places the views again and writes the items into the preset; answers the item, whether it is
/// shown and whether it was written. `canvas_place`, the board's drags and arrow keys.
pub(super) fn put(core: &Core, placement: Placement) -> Result<Value, CommandError> {
    let host = core.host();
    let (spec, area) = mode_and_area(host, LayoutMode::Canvas)?;
    let ModeKind::Canvas { snap, .. } = spec.kind else {
        return Err(not_in(LayoutMode::Canvas));
    };
    let sizes = host.mode_sizes();
    let (mut items, _) = canvas_state(&spec, &sizes, area);
    let Placement {
        index,
        x,
        y,
        zoom,
        snap: snapping,
    } = placement;
    let (Some(breakpoint), Some(size)) = (
        items.get(index).map(|item| item.breakpoint.clone()),
        sizes.get(index),
    ) else {
        return Err(CommandError::InvalidArgs(format!(
            "{index} is no view of the canvas"
        )));
    };
    let (width, height) = (f64::from(size.width) * zoom, f64::from(size.height) * zoom);
    let others: Vec<WorldRect> = items
        .iter()
        .zip(&sizes)
        .enumerate()
        .filter(|(other, _)| snapping && *other != index)
        .map(|(_, (item, size))| canvas::outer(item, size, LABEL_HEIGHT))
        .collect();
    let grid = if snapping { snap } else { 0 };
    let (x, y) = canvas::snap((x, y), (width, height), &others, grid, LABEL_HEIGHT);
    items[index] = CanvasItem {
        breakpoint: breakpoint.clone(),
        x: x.clamp(-MAX_CANVAS_COORD, MAX_CANVAS_COORD),
        y: y.clamp(-MAX_CANVAS_COORD, MAX_CANVAS_COORD),
        zoom,
    };
    let placed = items[index].clone();
    host.update_mode(|spec| {
        if let ModeKind::Canvas { items: current, .. } = &mut spec.kind {
            current.clone_from(&items);
        }
    });
    host.update_shell_state(|shell| shell.modes.fields.items.clone_from(&items));
    let persisted = persist_session(core);
    let visible = host
        .mode_spec()
        .map(|spec| spec.arrange(&host.mode_sizes(), area, LABEL_HEIGHT))
        .and_then(|slots| {
            slots
                .get(index)
                .map(|slot| slot.visible && contains(area, slot.outer()))
        })
        .unwrap_or(false);
    Ok(json!({
        "breakpoint": breakpoint,
        "x": placed.x,
        "y": placed.y,
        "zoom": placed.zoom,
        "visible": visible,
        "persisted": persisted,
    }))
}

/// The place in the canvas's order (below `len`) of `view` (a view id or a breakpoint id), else
/// of the active view.
pub(super) fn canvas_index(
    host: &Host,
    spec: &ModeSpec,
    view: Option<&Value>,
    len: usize,
) -> Result<usize, CommandError> {
    let breakpoint = target_breakpoint(host, view)?;
    spec.breakpoints
        .iter()
        .position(|id| *id == breakpoint)
        .filter(|index| *index < len)
        .ok_or_else(|| {
            CommandError::InvalidArgs(format!("{breakpoint} is not a view of the canvas"))
        })
}

/// The breakpoint of `view` (a view id or a breakpoint id), else the active view's.
fn target_breakpoint(host: &Host, view: Option<&Value>) -> Result<String, CommandError> {
    match view {
        None | Some(Value::Null) => active_breakpoint(host)
            .ok_or_else(|| CommandError::Failed("there is no view".to_owned())),
        Some(Value::String(breakpoint)) => Ok(breakpoint.clone()),
        Some(Value::Number(number)) => {
            let id = number
                .as_u64()
                .and_then(|id| u32::try_from(id).ok())
                .map(ViewId)
                .ok_or_else(|| CommandError::InvalidArgs(format!("{number} is no view id")))?;
            host.views()
                .into_iter()
                .find(|info| info.id == id)
                .and_then(|info| info.cell_breakpoint)
                .ok_or_else(|| CommandError::InvalidArgs(format!("view {number} does not exist")))
        }
        Some(other) => Err(CommandError::InvalidArgs(format!(
            "view must be a view id or a breakpoint id, not {other}"
        ))),
    }
}

/// The place of the active view in the mode's order.
pub(super) fn active_index(host: &Host) -> Option<usize> {
    let breakpoint = active_breakpoint(host)?;
    host.mode_spec()?
        .breakpoints
        .iter()
        .position(|id| *id == breakpoint)
}

#[cfg(test)]
#[path = "canvas_ops_tests.rs"]
mod tests;
