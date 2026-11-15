//! The canvas while a gesture moves it (RB-080-50): the middle button's pan
//! (`super::middle_pan`) and the wheel's zoom (`super::canvas_zoom`) try a camera out frame by
//! frame and keep it once, as the board's own drags do (`super::canvas_board`). A frame only
//! puts the views where the camera would show them ([`show`]: the window moves the overlays,
//! the board gets its frames and labels for the same camera as `canvasLive`) - no spec is
//! changed, no view is applied anew, no state goes out. The end keeps the camera ([`keep`],
//! one `set_camera`), a cancel puts everything back ([`back`]).

use rb_commands::CommandError;
use rb_config::LayoutMode;
use rb_ipc::CoreToShell;

use super::canvas::Camera;
use super::canvas_board::{board_state_at, view_ids};
use super::canvas_ops::{canvas_state, mode_and_area, set_camera};
use crate::control::Core;
use crate::host::LABEL_HEIGHT;
use crate::host::ui::PlacedView;

/// The canvas's camera in effect; an error outside canvas mode.
pub fn camera(core: &Core) -> Result<Camera, CommandError> {
    let host = core.host();
    let (spec, area) = mode_and_area(host, LayoutMode::Canvas)?;
    Ok(canvas_state(&spec, &host.mode_sizes(), area).1)
}

/// Shows the canvas through `camera` without keeping it: the views and the board's frames and
/// labels, for the same camera.
pub fn show(core: &Core, camera: Camera) -> Result<(), CommandError> {
    let host = core.host();
    let (spec, area) = mode_and_area(host, LayoutMode::Canvas)?;
    let sizes = host.mode_sizes();
    let (items, _) = canvas_state(&spec, &sizes, area);
    let camera = camera.within(super::canvas::camera_limit(&items));
    let placed = spec
        .canvas_slots(&items, &sizes, area, camera, LABEL_HEIGHT)
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
    host.place_live(area, placed);
    let board = board_state_at(host, &spec, area, &host.shell_state().modes, camera);
    core.send(&CoreToShell::CanvasLive { board });
    Ok(())
}

/// Keeps `camera`: the gesture ended.
pub fn keep(core: &Core, camera: Camera) -> Result<(), CommandError> {
    mode_and_area(core.host(), LayoutMode::Canvas)?;
    set_camera(core, camera);
    Ok(())
}

/// Puts the canvas back to the camera in effect: the gesture was cancelled.
pub fn back(core: &Core) {
    core.host().update_mode(|_| {});
}

#[cfg(test)]
#[path = "canvas_live_tests.rs"]
mod tests;
