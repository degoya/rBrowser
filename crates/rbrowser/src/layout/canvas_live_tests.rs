use serde_json::json;

use super::*;
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
    assert_eq!(camera(core).expect("a camera"), tried);
}

#[tokio::test]
async fn outside_the_canvas_there_is_nothing_to_show() {
    let grid = setup(config());
    assert!(camera(&grid.core).is_err());
    assert!(show(&grid.core, Camera::default()).is_err());
    assert!(keep(&grid.core, Camera::default()).is_err());
}
