//! The canvas while a gesture moves it (RB-080-50): the middle button's pan
//! (`super::middle_pan`) and the wheel's zoom (`super::canvas_zoom`) try a camera out frame by
//! frame and keep it once, as the board's own drags do (`super::canvas_board`). A frame only
//! puts the views where the camera would show them ([`show`]: the window moves the overlays,
//! the board gets its frames and labels for the same camera as `canvasLive`) - no spec is
//! changed, no view is applied anew, no state goes out. The end keeps the camera ([`keep`],
//! one `set_camera`), a cancel puts everything back ([`back`]).
//!
//! With a board that answers, a frame's views wait until the board painted its canvas
//! ([`painted`], `super::canvas_hold`, RB-090-55): views, frames, labels and the frame around
//! the footers move together. Until the gesture ends every state draws the board for the
//! camera on trial. The board's own drags go the same way (RB-094-18): a pan is a camera on
//! trial, a label dragged a place on trial for its view ([`show_moved`]).
//!
//! The pace of the frames is logged at `debug` under the target [`PACE`]: each frame sent to
//! the board (`seq`, whether its views wait), each answer (`placed` false for one too old), each
//! hold placed without an answer, and in the window each placement with the time it waited for
//! the UI thread and the stale ones it skipped.

use rb_commands::CommandError;
use rb_config::LayoutMode;
use rb_engine::{CellRect, ViewId};
use rb_ipc::CoreToShell;

use super::ModeSpec;
use super::canvas::Camera;
use super::canvas_board::{board_state_at, view_ids};
use super::canvas_hold::Moved;
use super::canvas_ops::{canvas_state, mode_and_area, set_camera};
use crate::control::Core;
use crate::host::label_height;
use crate::host::ui::PlacedView;

/// The log target of the canvas's frames and placements (RB-094-18).
pub const PACE: &str = "rbrowser::canvas_pace";

/// The canvas's camera in effect; an error outside canvas mode.
pub fn camera(core: &Core) -> Result<Camera, CommandError> {
    let host = core.host();
    let (spec, area) = mode_and_area(host, LayoutMode::Canvas)?;
    Ok(canvas_state(&spec, &host.mode_sizes(), area).1)
}

/// Shows the canvas through `camera` without keeping it: the views and the board's frames and
/// labels, for the same camera - the views once the board painted it, when it answers.
pub fn show(core: &Core, camera: Camera) -> Result<(), CommandError> {
    let host = core.host();
    let (spec, area) = mode_and_area(host, LayoutMode::Canvas)?;
    let sizes = host.mode_sizes();
    let (items, _) = canvas_state(&spec, &sizes, area);
    let camera = camera.within(super::canvas::camera_limit(&items));
    let placed: Vec<PlacedView> = spec
        .canvas_slots(&items, &sizes, area, camera, label_height())
        .iter()
        .zip(view_ids(host, &spec))
        .filter_map(|(slot, view)| {
            Some(PlacedView {
                view: view?,
                outer: slot.outer(),
                visible: slot.visible,
            })
        })
        .collect();
    host.try_out(camera);
    send_frame(core, &spec, area, placed, camera);
    Ok(())
}

/// Shows `view`, the canvas's view at `moved.index`, at the canvas place of `moved` without
/// keeping it (a label dragged, RB-094-18): the view and the board's frames and labels for the
/// same place - the view once the board painted it, when it answers.
pub fn show_moved(core: &Core, view: ViewId, moved: Moved) -> Result<(), CommandError> {
    let host = core.host();
    let (spec, area) = mode_and_area(host, LayoutMode::Canvas)?;
    let sizes = host.mode_sizes();
    let (mut items, camera) = canvas_state(&spec, &sizes, area);
    let camera = host.live_camera().unwrap_or(camera);
    let Some(item) = items.get_mut(moved.index) else {
        return Err(CommandError::InvalidArgs(format!(
            "{} is no view of the canvas",
            moved.index
        )));
    };
    item.x = moved.x;
    item.y = moved.y;
    let slots = spec.canvas_slots(&items, &sizes, area, camera, label_height());
    let placed = slots
        .get(moved.index)
        .map(|slot| PlacedView {
            view,
            outer: slot.outer(),
            visible: slot.visible,
        })
        .into_iter()
        .collect();
    host.try_moved(moved);
    send_frame(core, &spec, area, placed, camera);
    Ok(())
}

/// One frame of a gesture: `placed` held for the board's answer (or placed at once without
/// one) and the board's frames for it sent.
fn send_frame(
    core: &Core,
    spec: &ModeSpec,
    area: CellRect,
    placed: Vec<PlacedView>,
    camera: Camera,
) {
    let host = core.host();
    let seq = host.canvas_number();
    let held = host.board_acks();
    if held {
        host.hold(seq, area, placed);
    } else {
        host.place_live(area, placed);
    }
    let board = board_state_at(host, spec, area, &host.shell_state().modes, camera);
    core.send(&CoreToShell::CanvasLive { board, seq });
    tracing::debug!(target: PACE, seq, held, "a canvas frame was sent to the board");
}

/// The board painted the canvas `seq` (`canvasPainted`): its views go where it drew them.
pub fn painted(core: &Core, seq: u32) {
    core.host().painted(seq);
}

/// Keeps `camera`: the gesture ended.
pub fn keep(core: &Core, camera: Camera) -> Result<(), CommandError> {
    mode_and_area(core.host(), LayoutMode::Canvas)?;
    set_camera(core, camera);
    Ok(())
}

/// Puts the canvas back to the camera in effect: the gesture was cancelled.
pub fn back(core: &Core) {
    // The shell's pages end the board's trial with the next state, and the forwarder sends no
    // state like the one sent last (RB-090-36): the one after this goes out in any case.
    crate::ipc::push::force_next(core.host());
    core.host().update_mode(|_| {});
}

#[cfg(test)]
#[path = "canvas_live_tests.rs"]
mod tests;
