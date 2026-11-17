//! Ctrl + mouse wheel on the canvas (RB-080-50): the canvas zooms around the pointer, the
//! views scaled with it - the camera's zoom, as `layout.zoom_to_fit` and the canvas's other
//! commands set it, never the zoom of the shell's pages. The board sends the wheel's whole turn
//! since the gesture began (`canvasZoom`), the window the same for a wheel over a native strip
//! (`cef_app/board_pan.rs`): a turn in progress is only shown (`super::canvas_live`), the
//! camera is kept once the wheel came to rest. `layoutModes.wheelZoom` off leaves the canvas
//! as it is.

use rb_commands::CommandError;
use rb_ipc::DragPhase;

use super::canvas::Camera;
use super::canvas_board::checked;
use super::canvas_live;
use crate::control::Core;

/// The wheel's turn of one notch, in CSS px.
pub const NOTCH: f64 = 100.0;
/// How much one notch zooms.
pub const NOTCH_FACTOR: f64 = 1.1;
/// The largest turn of one gesture, in CSS px: from one end of the camera's range to the other
/// and some more.
pub const MAX_TURN: i32 = 4000;
/// How long the wheel rests before its zoom is kept, in ms (the board and the window wait it).
pub const REST_MS: u32 = 150;

/// `camera` zoomed by a wheel turn of `delta` CSS px (negative zooms in) around `at`, a point
/// in screen DIP from the area's top-left corner: the canvas point under it stays under it. The
/// zoom stays in the camera's range and at `max` at most (`canvas::camera_limit`).
pub fn zoomed_at(camera: Camera, at: (i32, i32), delta: i32, max: f64) -> Camera {
    let camera = camera.within(max);
    let turn = f64::from(delta.clamp(-MAX_TURN, MAX_TURN));
    let zoomed = Camera {
        zoom: camera.zoom * NOTCH_FACTOR.powf(-turn / NOTCH),
        ..camera
    }
    .within(max);
    to_zoom_at(camera, at, zoomed.zoom)
}

/// `camera` at `zoom` around `at` (screen DIP from the area's top-left corner): the canvas point
/// under it stays under it. At an end of the range nothing moves.
pub fn to_zoom_at(camera: Camera, at: (i32, i32), zoom: f64) -> Camera {
    let camera = camera.clamped();
    let zoom = Camera { zoom, ..camera }.clamped().zoom;
    if zoom == camera.zoom {
        return camera;
    }
    let (x, y) = (f64::from(at.0), f64::from(at.1));
    Camera {
        x: camera.x + x / camera.zoom - x / zoom,
        y: camera.y + y / camera.zoom - y / zoom,
        zoom,
    }
}

/// The wheel turned `delta` CSS px in all since its gesture began, with Ctrl held at `x`/`y`
/// (DIP from the board's corner): `Move` shows the canvas zoomed, `End` keeps it, `Cancel` goes
/// back.
pub fn wheel(
    core: &Core,
    x: i32,
    y: i32,
    delta: i32,
    phase: DragPhase,
) -> Result<(), CommandError> {
    checked(x, y)?;
    let camera = canvas_live::camera(core)?;
    if !core.user_settings().layout_modes.wheel_zoom {
        return Ok(());
    }
    let zoomed = zoomed_at(
        camera,
        (x, y),
        delta,
        super::canvas_ops::camera_limit(core.host()),
    );
    match phase {
        DragPhase::Move => canvas_live::show(core, zoomed),
        DragPhase::End => {
            if zoomed == camera.clamped() {
                // Nothing to keep; the views and the board's frames leave the last frame's
                // camera (RB-090-55).
                canvas_live::back(core);
                Ok(())
            } else {
                canvas_live::keep(core, zoomed)
            }
        }
        DragPhase::Cancel => {
            canvas_live::back(core);
            Ok(())
        }
    }
}

#[cfg(test)]
#[path = "canvas_zoom_tests.rs"]
mod tests;
