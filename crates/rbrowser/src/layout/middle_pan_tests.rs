use serde_json::json;

use super::*;
use crate::host::ui::recording::UiCall;
use crate::layout::canvas_ops::set_camera;
use crate::layout::test_support::{config, setup};

#[test]
fn a_press_dragged_past_the_slop_pans_and_the_release_keeps_it() {
    let mut pan = MiddlePan::default();
    assert_eq!(pan.moved((5, 5), true, false), Step::Pass, "no press yet");
    assert_eq!(pan.down((100, 80), true), Step::Begin);
    assert!(pan.active());
    // Within the slop it is not a drag yet.
    assert_eq!(pan.moved((104, 76), true, false), Step::Pass);
    assert_eq!(
        pan.moved((110, 70), true, false),
        Step::Move { dx: 10, dy: -10 }
    );
    // Back near the press it stays a drag.
    assert_eq!(
        pan.moved((101, 80), true, false),
        Step::Move { dx: 1, dy: 0 }
    );
    assert_eq!(
        pan.up((60, 80)),
        Step::End {
            dx: -40,
            dy: 0,
            taken: true
        }
    );
    assert!(!pan.active());
    assert_eq!(pan.up((60, 80)), Step::Pass, "a release without a press");
}

// Review of RB-080-50: with the pan on, a page on the canvas still gets its middle click.
#[test]
fn a_press_let_go_within_the_slop_is_the_pages_click_after_all() {
    let mut pan = MiddlePan::default();
    pan.down((100, 80), true);
    assert_eq!(pan.moved((102, 83), true, false), Step::Pass);
    assert_eq!(pan.up((103, 83)), Step::Click { at: (100, 80) });
    assert!(!pan.active());
    // A double click's second press is a press like the first.
    assert_eq!(pan.down((100, 80), true), Step::Begin);
    assert_eq!(pan.up((100, 80)), Step::Click { at: (100, 80) });
}

#[test]
fn a_press_elsewhere_is_the_pages() {
    let mut pan = MiddlePan::default();
    assert_eq!(pan.down((100, 80), false), Step::Pass);
    assert!(!pan.active());
    assert_eq!(pan.moved((150, 70), true, false), Step::Pass);
    assert_eq!(pan.up((150, 70)), Step::Pass);
}

#[test]
fn a_release_the_window_did_not_hear_ends_the_gesture_at_the_next_move() {
    let mut pan = MiddlePan::default();
    pan.down((0, 0), true);
    pan.moved((30, 40), true, false);
    assert_eq!(
        pan.moved((30, 40), false, false),
        Step::End {
            dx: 30,
            dy: 40,
            taken: false
        }
    );
    assert!(!pan.active());
    // Before it became a drag there is nothing to keep and no click to give.
    pan.down((0, 0), true);
    assert_eq!(pan.moved((1, 1), false, false), Step::Pass);
    assert!(!pan.active());
}

#[test]
fn escape_puts_a_drag_back_and_the_release_after_it_is_still_taken() {
    let mut pan = MiddlePan::default();
    pan.down((0, 0), true);
    pan.moved((30, 0), true, false);
    assert_eq!(pan.moved((40, 0), true, true), Step::Cancel);
    assert!(pan.active(), "the button is still down");
    assert_eq!(
        pan.moved((90, 0), true, false),
        Step::Pass,
        "no second drag"
    );
    assert_eq!(
        pan.up((90, 0)),
        Step::Take,
        "the page saw no press: no release either"
    );
    assert!(!pan.active());
    // Escape before the drag began: no click for the page, the release is taken.
    pan.down((0, 0), true);
    assert_eq!(pan.moved((1, 0), true, true), Step::Pass);
    assert_eq!(pan.up((1, 0)), Step::Take);
}

#[test]
fn a_drag_across_the_whole_range_does_not_overflow() {
    let mut pan = MiddlePan::default();
    pan.down((i32::MIN, i32::MIN), true);
    assert_eq!(
        pan.moved((i32::MAX, i32::MAX), true, false),
        Step::Move {
            dx: i32::MAX,
            dy: i32::MAX
        }
    );
}

// Review of RB-080-50: a frame of the drag is a placement, the release one kept camera.
#[tokio::test]
async fn a_frame_shows_the_canvas_dragged_and_only_the_release_keeps_it() {
    let canvas = setup(config());
    let core = &canvas.core;
    assert!(
        show(core, Camera::default(), 10, 10).is_err(),
        "the grid has no canvas"
    );
    crate::presets::run_command(core, "layout.select_preset", &json!({ "id": "board" }))
        .expect("a presets command")
        .expect("selects");
    let from = Camera {
        x: 100.0,
        y: 50.0,
        zoom: 0.5,
    };
    set_camera(core, from);
    let calls = canvas.ui.calls().len();
    show(core, from, 20, -10).expect("shown");
    show(core, from, 40, -10).expect("shown");
    let frames = canvas.ui.calls()[calls..].to_vec();
    assert!(
        frames.iter().all(|call| match call {
            UiCall::Place(_) => true,
            UiCall::Shell(script) => script.contains("canvasLive"),
            _ => false,
        }),
        "{frames:?}"
    );
    assert_eq!(
        canvas_live::camera(core).expect("a camera"),
        from,
        "nothing is kept yet"
    );
    // Each move counts from where the drag began: 40 DIP right at zoom 0.5 is 80 canvas DIP.
    keep(core, from, 40, -10).expect("kept");
    let now = canvas_live::camera(core).expect("a camera");
    assert_eq!((now.x, now.y, now.zoom), (20.0, 70.0, 0.5));
    keep(core, from, i32::MAX, i32::MIN).expect("kept");
    assert!(canvas_live::camera(core).expect("a camera").x.is_finite());
}
