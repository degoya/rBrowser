//! The canvas board (RB-080-04): in canvas mode a browser view of its own fills the grid area
//! behind the views' overlays (`cef_app/canvas_board.rs`) and shows [`BOARD_URL`]. CEF Views
//! panels get no mouse events, so the board draws each view's label strip (the overlays then
//! hold only the view) and takes the drags:
//!
//! - a label dragged moves its view live - only its overlay, no rebuild, no reload, nothing
//!   kept - and on release snaps it to the grid and its neighbours' edges and writes it into the
//!   preset ([`super::canvas_ops::put`], as `canvas_place` does); Escape puts it back;
//! - the empty board dragged pans the camera (live, kept on release, Escape goes back); with
//!   `layoutModes.spacePan` Space held turns a drag on a label into a pan too (the page decides);
//! - an arrow key on a focused label moves its view to the next line of the `snap` grid;
//! - a click on a label activates its view, a click on the active view's label opens its size
//!   quick pick under the strip, as in the other modes (RB-080-22, `cef_app/cell_labels.rs`);
//!   the strip shows the size as the grid's strips do;
//! - a click on the engine badge at a label's right end opens the view's engine menu under it
//!   (RB-080-33, `cef_app/engine_badge.rs`), as on a grid strip's badge;
//! - the label shows the pieces a grid strip shows beside the name - the address and the
//!   external engine's window, the latter in the engine badge while it shows (RB-090-71) - and
//!   a click on one does what it does on the strip (RB-090-69, [`crate::cell_info::board`]);
//!   the Web Vitals are the vitals bar's, the native strip under the view (RB-090-71);
//! - a right-click on a label opens the view menu, one on the free area the add menu
//!   (RB-080-41): the board says where (`canvasMenu`) and the core shows a native menu in front
//!   of the views (RB-080-46, `layout::view_menu_open`), the click a canvas point through the
//!   camera ([`board_point`]).
//!
//! The board sends its drags as screen DIP from where they began; the camera's zoom turns them
//! into canvas DIP. A view never goes past [`rb_config::MAX_CANVAS_COORD`].

use rb_commands::CommandError;
use rb_config::{LayoutMode, MAX_CANVAS_COORD};
use rb_engine::{CellRect, ViewHost as _, ViewId};
use rb_ipc::{
    BoardCamera, BoardItem, BoardState, CoreToShell, DragPhase, IpcErrorCode, LabelPiece, MapRect,
    ShellToCore,
};
use serde_json::{Value, json};

use super::ModeSpec;
use super::canvas::Camera;
use super::canvas_hold::Moved;
use super::canvas_live::PACE;
use super::canvas_ops::{Placement, canvas_index, canvas_state, mode_and_area, put, set_camera};
use super::spec::ModeSession;
use crate::control::Core;
use crate::host::{Host, label_height};
use crate::ipc::Failure;

/// The board's page.
pub const BOARD_URL: &str = "rbrowser://ui/canvas-board.html";
/// The largest drag read, in screen DIP: more than any screen.
pub const MAX_DRAG: i32 = 200_000;

/// What the board draws: each view's label strip and rect from the area's top-left corner -
/// through the camera a gesture is trying out while it lasts, a dragged view at the place it
/// tries out, so a state sent then does not take the frames back from the views (RB-090-55,
/// RB-094-18).
pub fn board_state(
    host: &Host,
    spec: &ModeSpec,
    area: CellRect,
    session: &ModeSession,
) -> BoardState {
    let camera = host
        .live_camera()
        .unwrap_or_else(|| canvas_state(spec, &host.mode_sizes(), area).1);
    board_state_at(host, spec, area, session, camera)
}

/// [`board_state`] as `camera` would show it - the camera in effect, or one a gesture is trying
/// out (`super::canvas_live`).
pub fn board_state_at(
    host: &Host,
    spec: &ModeSpec,
    area: CellRect,
    session: &ModeSession,
    camera: Camera,
) -> BoardState {
    let sizes = host.mode_sizes();
    let (mut items, _) = canvas_state(spec, &sizes, area);
    if let Some(moved) = host.live_moved()
        && let Some(item) = items.get_mut(moved.index)
    {
        item.x = moved.x;
        item.y = moved.y;
    }
    let slots = spec.canvas_slots(&items, &sizes, area, camera, label_height());
    let level = super::canvas_level::level_at(host, &items, camera).unwrap_or(0.0);
    let infos = host.cell_infos();
    let piece = |view, kind| crate::cell_info::board::board_piece_of(&infos, view, kind);
    let items = slots
        .iter()
        .zip(view_ids(host, spec))
        .filter_map(|(slot, view)| {
            let view = view?;
            Some(BoardItem {
                view,
                label: from_corner(slot.label, area),
                rect: from_corner(slot.rect, area),
                visible: slot.visible,
                text: host.cell_label_text(view).unwrap_or_default(),
                page_zoom: crate::page_zoom::badge(view),
                address: piece(view, LabelPiece::Address),
                engine_status: piece(view, LabelPiece::EngineStatus),
            })
        })
        .collect();
    BoardState {
        space_pan: session.settings.space_pan,
        wheel_zoom: session.settings.wheel_zoom,
        items,
        camera: board_camera(camera),
        level_permille: u32::try_from(whole(level * 1000.0)).unwrap_or(0),
    }
}

/// `camera` as the board reads it: rounded to whole DIP, the zoom in per mille (at least 1).
pub fn board_camera(camera: Camera) -> BoardCamera {
    let camera = camera.clamped();
    BoardCamera {
        x: whole(camera.x),
        y: whole(camera.y),
        zoom_permille: u32::try_from(whole(camera.zoom * 1000.0))
            .unwrap_or(1000)
            .max(1),
    }
}

/// The canvas point of the board's `x`/`y` (DIP from its top-left corner) through `camera`.
pub fn board_point(camera: BoardCamera, x: i32, y: i32) -> (f64, f64) {
    let zoom = f64::from(camera.zoom_permille.max(1)) / 1000.0;
    (
        f64::from(camera.x) + f64::from(x) / zoom,
        f64::from(camera.y) + f64::from(y) / zoom,
    )
}

/// The board's requests (`crate::features`); `None` for every other message.
pub fn ipc(core: &Core, message: &ShellToCore) -> Option<Result<CoreToShell, Failure>> {
    let done = match *message {
        ShellToCore::CanvasDrag {
            view,
            dx,
            dy,
            phase,
        } => drag(core, view, dx, dy, phase),
        ShellToCore::CanvasPan { dx, dy, phase } => pan(core, dx, dy, phase),
        ShellToCore::CanvasStep { view, dx, dy } => step(core, view, dx, dy).map(|_| ()),
        ShellToCore::CanvasSizePick { view, force } => size_pick(core, view, force),
        ShellToCore::CanvasEnginePick { view } => engine_pick(core, view),
        ShellToCore::CanvasPiece { view, piece } => {
            crate::cell_info::board::click(core, view, piece)
        }
        ShellToCore::CanvasZoom { x, y, delta, phase } => {
            super::canvas_zoom::wheel(core, x, y, delta, phase)
        }
        ShellToCore::CanvasPainted { seq } => {
            super::canvas_live::painted(core, seq);
            Ok(())
        }
        // The right-click's menus and the toolbar's "+" (RB-080-46).
        _ => return super::view_menu_open::ipc(core, message),
    };
    if let Err(error) = &done {
        tracing::warn!(%error, "a request of the canvas board failed");
    }
    Some(done.map(|()| CoreToShell::Ack).map_err(failure))
}

/// The label of `view` dragged `dx`/`dy` screen DIP: `move` shows it there (its frame on the
/// board, the view once the board painted it - `super::canvas_live::show_moved`, RB-094-18),
/// `end` keeps it (snapped), `cancel` puts it back. The end and the cancel send the next state
/// even when it is the one sent last: it ends the board's trial.
pub fn drag(
    core: &Core,
    view: ViewId,
    dx: i32,
    dy: i32,
    phase: DragPhase,
) -> Result<(), CommandError> {
    checked(dx, dy)?;
    let host = core.host();
    let (spec, area) = mode_and_area(host, LayoutMode::Canvas)?;
    let sizes = host.mode_sizes();
    let (items, camera) = canvas_state(&spec, &sizes, area);
    let index = canvas_index(
        host,
        &spec,
        Some(&json!(view.0)),
        items.len().min(sizes.len()),
    )?;
    let (x, y) = moved((items[index].x, items[index].y), (dx, dy), camera);
    match phase {
        DragPhase::Move => {
            tracing::debug!(target: PACE, %view, dx, dy, "a label drag moved");
            let trial = Moved {
                index,
                x: whole(x),
                y: whole(y),
            };
            super::canvas_live::show_moved(core, view, trial)?;
        }
        DragPhase::End => {
            tracing::info!(%view, dx, dy, "a view was dragged on the canvas board");
            crate::ipc::push::force_next(host);
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
            )?;
        }
        DragPhase::Cancel => super::canvas_live::back(core),
    }
    Ok(())
}

/// The empty board dragged `dx`/`dy` screen DIP: the canvas follows the pointer - `move` live
/// (a camera on trial, `super::canvas_live::show`, RB-094-18), `end` keeps the camera, `cancel`
/// goes back.
pub fn pan(core: &Core, dx: i32, dy: i32, phase: DragPhase) -> Result<(), CommandError> {
    checked(dx, dy)?;
    let host = core.host();
    let (spec, area) = mode_and_area(host, LayoutMode::Canvas)?;
    let (_, camera) = canvas_state(&spec, &host.mode_sizes(), area);
    let panned = panned(camera, (dx, dy));
    match phase {
        DragPhase::Move => {
            tracing::debug!(target: PACE, dx, dy, "a board pan moved");
            super::canvas_live::show(core, panned)?;
        }
        DragPhase::End => set_camera(core, panned),
        DragPhase::Cancel => super::canvas_live::back(core),
    }
    Ok(())
}

/// An arrow key on the label of `view`: the view moves `dx`/`dy` (-1, 0 or 1) to the next line
/// of the canvas's `snap` grid (1 DIP without one) on that axis and is kept.
pub fn step(core: &Core, view: ViewId, dx: i32, dy: i32) -> Result<Value, CommandError> {
    if !(-1..=1).contains(&dx) || !(-1..=1).contains(&dy) {
        return Err(CommandError::InvalidArgs(
            "a step is -1, 0 or 1 on each axis".into(),
        ));
    }
    let host = core.host();
    let (spec, area) = mode_and_area(host, LayoutMode::Canvas)?;
    let sizes = host.mode_sizes();
    let (items, _) = canvas_state(&spec, &sizes, area);
    let index = canvas_index(
        host,
        &spec,
        Some(&json!(view.0)),
        items.len().min(sizes.len()),
    )?;
    let snap = match spec.kind {
        super::spec::ModeKind::Canvas { snap, .. } => snap,
        _ => 0,
    };
    let item = &items[index];
    put(
        core,
        Placement {
            index,
            x: f64::from(stepped(item.x, dx, snap)),
            y: f64::from(stepped(item.y, dy, snap)),
            zoom: item.zoom,
            snap: false,
        },
    )
}

/// A click on the board's label of `view`: the view becomes active; was it active already, its
/// size quick pick opens under the label the board draws (as in the other modes, RB-080-22).
/// With `force` (the view menu's "Size...", RB-080-41) the pick opens and nothing is activated.
pub fn size_pick(core: &Core, view: ViewId, force: bool) -> Result<(), CommandError> {
    let host = core.host();
    let label = board_label(host, view)?;
    if force {
        host.ui().show_size_pick(view, label);
        return Ok(());
    }
    let was_active = host.active_view() == Some(view);
    host.set_active_view(view)
        .map_err(|error| CommandError::Failed(error.to_string()))?;
    if was_active {
        host.ui().show_size_pick(view, label);
    }
    Ok(())
}

/// A click on the engine badge the board draws for `view` (RB-080-33): its engine menu opens
/// under the right end of the label the board draws; the view need not be active.
pub fn engine_pick(core: &Core, view: ViewId) -> Result<(), CommandError> {
    let host = core.host();
    let label = board_label(host, view)?;
    host.ui().show_engine_pick(view, Some(label));
    Ok(())
}

/// The label strip the board draws for `view`, from the area's top-left corner.
fn board_label(host: &Host, view: ViewId) -> Result<MapRect, CommandError> {
    let (spec, area) = mode_and_area(host, LayoutMode::Canvas)?;
    spec.arrange(&host.mode_sizes(), area, label_height())
        .iter()
        .zip(view_ids(host, &spec))
        .find(|(_, on_board)| *on_board == Some(view))
        .map(|(slot, _)| from_corner(slot.label, area))
        .ok_or_else(|| CommandError::InvalidArgs(format!("view {view} is not on the canvas")))
}

/// Whether the board draws `view`.
pub fn on_board(host: &Host, spec: &ModeSpec, view: ViewId) -> bool {
    view_ids(host, spec).contains(&Some(view))
}

/// The canvas point `at` moved by `delta` screen DIP through `camera`, held to the canvas.
pub fn moved(at: (i32, i32), delta: (i32, i32), camera: Camera) -> (f64, f64) {
    let zoom = camera.clamped().zoom;
    let limit = f64::from(MAX_CANVAS_COORD);
    let axis = |at: i32, delta: i32| (f64::from(at) + f64::from(delta) / zoom).clamp(-limit, limit);
    (axis(at.0, delta.0), axis(at.1, delta.1))
}

/// `camera` after the board was dragged `delta` screen DIP: the canvas follows the pointer.
pub fn panned(camera: Camera, delta: (i32, i32)) -> Camera {
    let camera = camera.clamped();
    Camera {
        x: camera.x - f64::from(delta.0) / camera.zoom,
        y: camera.y - f64::from(delta.1) / camera.zoom,
        zoom: camera.zoom,
    }
}

/// `value` one step in `direction` (-1, 0, 1): to the next line of the `snap` grid that way, or
/// 1 DIP without a grid.
pub fn stepped(value: i32, direction: i32, snap: u32) -> i32 {
    if direction == 0 {
        return value;
    }
    let Ok(grid) = i32::try_from(snap.max(1)) else {
        return value;
    };
    let next = value.saturating_add(direction.saturating_mul(grid));
    // The nearest grid line to one step on; never back past where the view stood.
    let line = (f64::from(next) / f64::from(grid)).round() * f64::from(grid);
    whole(line).clamp(-MAX_CANVAS_COORD, MAX_CANVAS_COORD)
}

/// A canvas view's overlay (`cef_app/placed.rs`): `outer` - its label strip and view - less the
/// `trim` DIP at the top the board draws the strip in.
pub fn below_label(outer: CellRect, trim: u32) -> CellRect {
    let trim = trim.min(outer.height);
    CellRect {
        y: outer
            .y
            .saturating_add(i32::try_from(trim).unwrap_or(i32::MAX)),
        height: outer.height - trim,
        ..outer
    }
}

/// The view of each breakpoint of `spec`, in its order.
pub(super) fn view_ids(host: &Host, spec: &ModeSpec) -> Vec<Option<ViewId>> {
    let views = host.views();
    spec.breakpoints
        .iter()
        .map(|breakpoint| {
            views
                .iter()
                .find(|view| view.cell_breakpoint.as_deref() == Some(breakpoint.as_str()))
                .map(|view| view.id)
        })
        .collect()
}

fn from_corner(rect: CellRect, area: CellRect) -> MapRect {
    MapRect {
        x: rect.x.saturating_sub(area.x),
        y: rect.y.saturating_sub(area.y),
        width: rect.width,
        height: rect.height,
    }
}

/// Two screen distances a page sent - a drag, a pointer's place: each within [`MAX_DRAG`].
pub(super) fn checked(dx: i32, dy: i32) -> Result<(), CommandError> {
    let within = |delta: i32| delta.checked_abs().is_some_and(|delta| delta <= MAX_DRAG);
    if within(dx) && within(dy) {
        Ok(())
    } else {
        Err(CommandError::InvalidArgs(format!(
            "a drag or a point is at most {MAX_DRAG} DIP from where it counts"
        )))
    }
}

fn whole(value: f64) -> i32 {
    if value.is_finite() {
        // Held to the i32 range first.
        value
            .round()
            .clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32
    } else {
        0
    }
}

fn failure(error: CommandError) -> Failure {
    let code = match error {
        CommandError::InvalidArgs(_) | CommandError::Failed(_) => IpcErrorCode::BadRequest,
        CommandError::NotImplemented => IpcErrorCode::NotImplemented,
        _ => IpcErrorCode::Internal,
    };
    (code, error.to_string())
}

#[cfg(test)]
#[path = "canvas_board_tests.rs"]
mod tests;
