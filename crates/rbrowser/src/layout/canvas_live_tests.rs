use std::sync::atomic::Ordering;
use std::time::Duration;

use serde_json::json;

use super::*;
use crate::host::ui::PlacedView;
use crate::host::ui::recording::UiCall;
use crate::layout::test_support::{Setup, config, setup};

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
            x: 100.0,
            y: 50.0,
            zoom: 0.5,
        },
    );
    setup
}

/// What a frame may cost: the window's calls by kind since `from`.
fn kinds(setup: &Setup, from: usize) -> Vec<&'static str> {
    setup.ui.calls()[from..]
        .iter()
        .map(|call| match call {
            UiCall::Place(_) => "place",
            UiCall::Shell(script) if script.contains("canvasLive") => "live",
            UiCall::Shell(_) => "shell",
            _ => "other",
        })
        .collect()
}

// Review of RB-080-50: a frame of a gesture is a placement and the board's frames - not a
// committed camera with a state for every page and every view applied anew.
#[tokio::test]
async fn a_frame_places_the_views_and_the_boards_frames_and_keeps_nothing() {
    let canvas = board();
    let core = &canvas.core;
    let before = camera(core).expect("a camera");
    let from = canvas.ui.calls().len();
    let tried = Camera {
        x: 60.0,
        y: 70.0,
        zoom: 0.25,
    };
    show(core, tried).expect("shown");
    assert_eq!(kinds(&canvas, from), ["place", "live"]);
    assert_eq!(camera(core).expect("a camera"), before, "nothing is kept");
    let live = canvas
        .ui
        .calls()
        .into_iter()
        .rev()
        .find_map(|call| match call {
            UiCall::Shell(script) if script.contains("canvasLive") => Some(script),
            _ => None,
        })
        .expect("the board's frames");
    assert!(live.contains("\"zoomPermille\":250"), "{live}");
    // The end keeps it, once; a cancel after another frame goes back to what was kept.
    keep(core, tried).expect("kept");
    assert_eq!(camera(core).expect("a camera"), tried);
    show(core, before).expect("shown");
    let from = canvas.ui.calls().len();
    back(core);
    assert!(kinds(&canvas, from).contains(&"place"));
    // The next state goes out even when it is the one sent last: it ends the board's trial
    // (RB-090-36).
    assert!(crate::ipc::push::take_forced(core.host()));
    assert_eq!(camera(core).expect("a camera"), tried);
}

#[tokio::test]
async fn outside_the_canvas_there_is_nothing_to_show() {
    let grid = setup(config());
    assert!(camera(&grid.core).is_err());
    assert!(show(&grid.core, Camera::default()).is_err());
    assert!(keep(&grid.core, Camera::default()).is_err());
}

const TRIED: Camera = Camera {
    x: 60.0,
    y: 70.0,
    zoom: 0.25,
};

/// The window's board answers each canvas it painted (RB-090-55), as the CEF board does.
fn answering(setup: &Setup) {
    setup.ui.board_acks.store(true, Ordering::Relaxed);
}

/// The number of the last canvas the board was sent (`canvasLive`'s `seq`).
fn last_live(setup: &Setup) -> u32 {
    let script = setup
        .ui
        .calls()
        .into_iter()
        .rev()
        .find_map(|call| match call {
            UiCall::Shell(script) if script.contains("canvasLive") => Some(script),
            _ => None,
        })
        .expect("a canvas for the board");
    let at = script.find("\"seq\":").expect("a number") + "\"seq\":".len();
    script[at..]
        .chars()
        .take_while(char::is_ascii_digit)
        .collect::<String>()
        .parse()
        .expect("a number")
}

/// The window's placements since `from`.
fn places(setup: &Setup, from: usize) -> Vec<Vec<PlacedView>> {
    setup.ui.calls()[from..]
        .iter()
        .filter_map(|call| match call {
            UiCall::Place(placed) => Some(placed.clone()),
            _ => None,
        })
        .collect()
}

/// Where a frame at `camera` puts the views when no board answers.
fn placed_at(camera: Camera) -> Vec<PlacedView> {
    let canvas = board();
    let from = canvas.ui.calls().len();
    show(&canvas.core, camera).expect("shown");
    places(&canvas, from).pop().expect("placed at once")
}

/// The camera of the board the next state draws.
fn board_zoom(core: &Core) -> u32 {
    crate::layout::ipc::mode_state(core.host(), &core.host().shell_state().modes)
        .and_then(|state| state.board)
        .expect("the canvas has a board")
        .camera
        .zoom_permille
}

// RB-090-55: the window moved the views at once and the board drew their frames, labels and
// the frame around the footer frames later, in a page of its own: the views waited for nothing.
// With a board that answers, a frame's views go to their places only once the board painted
// that canvas.
#[tokio::test]
async fn with_a_board_that_answers_the_views_wait_for_their_frames() {
    let canvas = board();
    answering(&canvas);
    let core = &canvas.core;
    let from = canvas.ui.calls().len();
    show(core, TRIED).expect("shown");
    assert_eq!(
        kinds(&canvas, from),
        ["live"],
        "the views wait for the board"
    );
    let number = last_live(&canvas);
    painted(core, number);
    assert_eq!(places(&canvas, from), [placed_at(TRIED)]);
    painted(core, number);
    assert_eq!(places(&canvas, from).len(), 1, "placed once");
}

#[tokio::test]
async fn an_older_canvas_painted_after_a_newer_one_moves_nothing() {
    let canvas = board();
    answering(&canvas);
    let core = &canvas.core;
    let other = Camera { zoom: 0.4, ..TRIED };
    show(core, other).expect("shown");
    let first = last_live(&canvas);
    show(core, TRIED).expect("shown");
    let second = last_live(&canvas);
    let from = canvas.ui.calls().len();
    // The board's answer, as it sends it.
    for seq in [second, first] {
        let request =
            serde_json::to_string(&rb_ipc::ShellToCore::CanvasPainted { seq }).expect("encodes");
        let answer = crate::ipc::dispatch(core, &request).expect("answered");
        assert!(answer.contains("\"ack\""), "{answer}");
    }
    assert_eq!(places(&canvas, from), [placed_at(TRIED)]);
}

// The status bar's menu, the keys and MCP keep a camera at once; the views wait for the board's
// frames of it all the same, and a frame painted before the end moves nothing any more.
#[tokio::test]
async fn a_kept_camera_waits_for_the_board_too() {
    let canvas = board();
    answering(&canvas);
    let core = &canvas.core;
    show(core, Camera { zoom: 0.4, ..TRIED }).expect("shown");
    let trial = last_live(&canvas);
    let from = canvas.ui.calls().len();
    keep(core, TRIED).expect("kept");
    assert_eq!(camera(core).expect("a camera"), TRIED, "kept at once");
    assert_eq!(board_zoom(core), 250);
    assert!(
        places(&canvas, from).is_empty(),
        "the views wait for the board"
    );
    painted(core, trial);
    assert!(places(&canvas, from).is_empty(), "the trial ended");
    painted(core, last_live(&canvas));
    assert_eq!(places(&canvas, from).len(), 1);
    let from = canvas.ui.calls().len();
    crate::layout::canvas_level::run(core, crate::layout::canvas_level::ZOOM_IN, &json!(null))
        .expect("zooms in");
    assert!(places(&canvas, from).is_empty(), "the zoom keys wait too");
    painted(core, last_live(&canvas));
    assert_eq!(places(&canvas, from).len(), 1);
}

// A board that does not answer - not loaded yet, its page gone - never keeps the views from
// their places.
#[tokio::test]
async fn a_board_that_does_not_answer_never_keeps_the_views() {
    let canvas = board();
    answering(&canvas);
    let from = canvas.ui.calls().len();
    show(&canvas.core, TRIED).expect("shown");
    assert!(places(&canvas, from).is_empty());
    tokio::time::timeout(Duration::from_secs(10), async {
        while places(&canvas, from).is_empty() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("the views go to their places without the board");
    assert_eq!(places(&canvas, from), [placed_at(TRIED)]);
}

// RB-090-55: every frame resized the views, their new metrics sent a state to the board, and
// that state drew the frames of the camera kept before the gesture - the frames jumped back
// until the gesture ended. A state during a gesture draws the camera on trial.
#[tokio::test]
async fn a_state_during_a_gesture_draws_the_camera_on_trial() {
    let canvas = board();
    let core = &canvas.core;
    assert_eq!(board_zoom(core), 500);
    show(core, TRIED).expect("shown");
    assert_eq!(board_zoom(core), 250);
    let state = crate::ipc::state_event(core.host()).expect("a state");
    assert!(state.contains("\"zoomPermille\":250"), "{state}");
    back(core);
    assert_eq!(board_zoom(core), 500, "a cancel ends the trial");
    show(core, TRIED).expect("shown");
    crate::presets::run_command(core, "layout.select_preset", &json!({ "id": "pair" }))
        .expect("a presets command")
        .expect("selects");
    assert_eq!(core.host().live_camera(), None, "another mode ends it too");
}
