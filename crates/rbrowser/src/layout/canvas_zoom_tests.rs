use rb_ipc::{DragPhase, ShellToCore};
use serde_json::json;

use super::*;
use crate::host::ui::recording::UiCall;
use crate::layout::canvas::ZOOM_RANGE;
use crate::layout::canvas_ops::set_camera;
use crate::layout::test_support::{Setup, config, setup};

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

#[test]
fn a_notch_zooms_by_a_tenth_and_keeps_the_point_under_the_pointer() {
    let camera = Camera {
        x: 100.0,
        y: 50.0,
        zoom: 0.5,
    };
    let at = (300, 200);
    // The canvas point under the pointer before: 100 + 300 / 0.5, 50 + 200 / 0.5.
    let world = (700.0, 450.0);
    for delta in [-100, 100, -250, 37] {
        let zoomed = zoomed_at(camera, at, delta, ZOOM_RANGE.1);
        assert!(
            close(zoomed.x + f64::from(at.0) / zoomed.zoom, world.0)
                && close(zoomed.y + f64::from(at.1) / zoomed.zoom, world.1),
            "{delta}: {zoomed:?}"
        );
    }
    assert!(
        close(zoomed_at(camera, at, -100, ZOOM_RANGE.1).zoom, 0.55),
        "in"
    );
    assert!(
        close(zoomed_at(camera, at, 100, ZOOM_RANGE.1).zoom, 0.5 / 1.1),
        "out"
    );
    assert_eq!(zoomed_at(camera, at, 0, ZOOM_RANGE.1), camera);
}

#[test]
fn the_zoom_stays_in_the_cameras_range_whatever_the_wheel_says() {
    let camera = Camera {
        x: 0.0,
        y: 0.0,
        zoom: 0.9,
    };
    let near = zoomed_at(camera, (10, 10), i32::MIN, ZOOM_RANGE.1);
    assert!(near.zoom <= ZOOM_RANGE.1 && near.x.is_finite() && near.y.is_finite());
    let mut far = camera;
    for _ in 0..200 {
        far = zoomed_at(far, (10, 10), i32::MAX, ZOOM_RANGE.1);
    }
    assert!(close(far.zoom, ZOOM_RANGE.0), "{far:?}");
    // At the end of the range another turn moves nothing.
    assert_eq!(zoomed_at(far, (500, 500), 100, ZOOM_RANGE.1), far);
}

fn board() -> Setup {
    let setup = setup(config());
    crate::presets::run_command(
        &setup.core,
        "layout.select_preset",
        &json!({ "id": "board" }),
    )
    .expect("a presets command")
    .expect("selects");
    set_camera(
        &setup.core,
        Camera {
            x: 0.0,
            y: 0.0,
            zoom: 0.5,
        },
    );
    setup
}

fn zoom_of(core: &Core) -> f64 {
    canvas_live::camera(core).expect("a camera").zoom
}

fn send(core: &Core, delta: i32, phase: DragPhase) -> String {
    let message = ShellToCore::CanvasZoom {
        x: 40,
        y: 30,
        delta,
        phase,
    };
    let request = serde_json::to_string(&message).expect("encodes");
    crate::ipc::dispatch(core, &request).unwrap_or_else(|(_, refusal)| format!("error: {refusal}"))
}

// RB-080-50: the wheel's zoom is the camera's - the views are placed scaled. A turn in
// progress is only shown; the camera is kept when the wheel came to rest.
#[tokio::test]
async fn the_wheel_shows_the_zoom_while_it_turns_and_keeps_it_at_rest() {
    let canvas = board();
    let core = &canvas.core;
    let from = canvas.ui.calls().len();
    assert!(send(core, -100, DragPhase::Move).contains("\"ack\""));
    assert!(send(core, -200, DragPhase::Move).contains("\"ack\""));
    assert!(close(zoom_of(core), 0.5), "nothing is kept while it turns");
    let frames: Vec<UiCall> = canvas.ui.calls()[from..].to_vec();
    assert_eq!(
        frames
            .iter()
            .filter(|call| matches!(call, UiCall::Place(_)))
            .count(),
        2,
        "each frame places the views: {frames:?}"
    );
    assert!(
        frames.iter().all(|call| match call {
            UiCall::Place(_) => true,
            UiCall::Shell(script) => script.contains("canvasLive"),
            _ => false,
        }),
        "a frame is a placement and the board's frames, nothing else: {frames:?}"
    );
    // The whole turn counts from where the gesture began: two notches.
    assert!(send(core, -200, DragPhase::End).contains("\"ack\""));
    assert!(close(zoom_of(core), 0.5 * 1.1 * 1.1), "{}", zoom_of(core));
    let board = crate::layout::ipc::mode_state(core.host(), &core.host().shell_state().modes)
        .and_then(|state| state.board)
        .expect("the canvas has a board");
    assert_eq!(board.camera.zoom_permille, 605);
    assert!(board.wheel_zoom);
    // A cancelled turn goes back.
    send(core, 300, DragPhase::Move);
    send(core, 300, DragPhase::Cancel);
    assert!(close(zoom_of(core), 0.605));
}

#[tokio::test]
async fn the_wheel_zooms_nothing_when_switched_off_or_outside_the_canvas() {
    let canvas = board();
    let core = &canvas.core;
    let mut user = core.user_settings();
    user.layout_modes.wheel_zoom = false;
    core.replace_user_settings(user);
    let from = canvas.ui.calls().len();
    for phase in [DragPhase::Move, DragPhase::End] {
        assert!(send(core, -100, phase).contains("\"ack\""));
    }
    assert!(close(zoom_of(core), 0.5));
    assert!(
        !canvas.ui.calls()[from..]
            .iter()
            .any(|call| matches!(call, UiCall::Place(_))),
        "nothing moves"
    );
    // In the grid there is no canvas to zoom; a pointer far off any screen is refused.
    let grid = setup(config());
    assert!(send(&grid.core, -100, DragPhase::Move).contains("error"));
    let far = serde_json::to_string(&ShellToCore::CanvasZoom {
        x: i32::MAX,
        y: 0,
        delta: 1,
        phase: DragPhase::Move,
    })
    .expect("encodes");
    assert!(crate::ipc::dispatch(core, &far).is_err());
}
