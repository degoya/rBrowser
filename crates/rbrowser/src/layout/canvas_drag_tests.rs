//! RB-094-18: a label or the board dragged moves its views and their frames together. The board
//! drew the frames at the pointer while the core moved the views on its own clock, one window
//! move per board request, never waiting for the board's paint: frame and view drifted apart
//! during every drag. With a board that answers, a drag's frame goes the way of a zoom's
//! (RB-090-55): the core sends the board its frames (`canvasLive`) and moves the views once the
//! board painted them, the newest frame only.

use std::sync::atomic::Ordering;

use rb_engine::{ViewHost as _, ViewId};
use rb_ipc::{BoardItem, DragPhase};
use serde_json::json;

use crate::control::Core;
use crate::host::ui::PlacedView;
use crate::host::ui::recording::UiCall;
use crate::layout::canvas::Camera;
use crate::layout::canvas_board::{drag, pan};
use crate::layout::canvas_live::painted;
use crate::layout::canvas_ops::set_camera;
use crate::layout::test_support::{AREA, Setup, config, setup};

fn view_of(core: &Core, breakpoint: &str) -> ViewId {
    core.host()
        .views()
        .into_iter()
        .find(|view| view.cell_breakpoint.as_deref() == Some(breakpoint))
        .unwrap_or_else(|| panic!("{breakpoint}"))
        .id
}

/// The board with the camera at the canvas origin, zoom 1; the window's board answers each
/// canvas it painted, as the CEF board does.
fn answering_board() -> Setup {
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
            zoom: 1.0,
        },
    );
    setup.ui.board_acks.store(true, Ordering::Relaxed);
    setup
}

/// The window's calls by kind since `from`.
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

/// Where the window put `place`, as the board reads it: the left edge and the view's top (below
/// the label band of `item`) from the area's top-left corner.
fn board_place(place: &PlacedView, item: &BoardItem) -> (i32, i32) {
    let band = i32::try_from(item.label.height).expect("a small height");
    (place.outer.x - AREA.x, place.outer.y + band - AREA.y)
}

/// What the next state's board draws for `view`.
fn board_item(core: &Core, view: ViewId) -> BoardItem {
    crate::layout::ipc::mode_state(core.host(), &core.host().shell_state().modes)
        .and_then(|state| state.board)
        .expect("the canvas has a board")
        .items
        .into_iter()
        .find(|item| item.view == view)
        .expect("the view is on the board")
}

#[tokio::test]
async fn with_a_board_that_answers_a_dragged_view_waits_for_its_frame() {
    let canvas = answering_board();
    let core = &canvas.core;
    let mobile = view_of(core, "mobile");
    let before = board_item(core, mobile);
    let from = canvas.ui.calls().len();
    // Down, not up: the view starts at the board's top edge, where the board clips its frame.
    drag(core, mobile, 30, 10, DragPhase::Move).expect("moves");
    assert_eq!(
        kinds(&canvas, from),
        ["live"],
        "the view waits for its frame"
    );
    let tried = board_item(core, mobile);
    assert_eq!(
        (tried.label.x, tried.label.y),
        (before.label.x + 30, before.label.y + 10)
    );
    assert_eq!(
        (tried.rect.x, tried.rect.y),
        (before.rect.x + 30, before.rect.y + 10)
    );
    painted(core, last_live(&canvas));
    let placed = places(&canvas, from);
    assert_eq!(placed.len(), 1, "placed once");
    let [place] = placed[0].as_slice() else {
        panic!("only the dragged view moves: {placed:?}");
    };
    assert_eq!(place.view, mobile);
    // Where the board drew its frame: the frame and the view are in one place.
    assert_eq!(board_place(place, &tried), (tried.label.x, tried.rect.y));
}

#[tokio::test]
async fn only_the_newest_frame_of_a_drag_moves_the_view() {
    let canvas = answering_board();
    let core = &canvas.core;
    let mobile = view_of(core, "mobile");
    let before = board_item(core, mobile);
    drag(core, mobile, 10, 0, DragPhase::Move).expect("moves");
    let first = last_live(&canvas);
    drag(core, mobile, 20, 0, DragPhase::Move).expect("moves");
    let second = last_live(&canvas);
    let from = canvas.ui.calls().len();
    painted(core, second);
    painted(core, first);
    let placed = places(&canvas, from);
    assert_eq!(placed.len(), 1, "the older frame moves nothing");
    assert_eq!(placed[0][0].outer.x - AREA.x, before.label.x + 20);
}

// A state during a drag (a page's title, its metrics) drew the view's frame back at its kept
// place; the end or a cancel ends the trial with a state of its own, also when it is the one
// sent last (RB-090-36).
#[tokio::test]
async fn a_state_during_a_drag_draws_the_dragged_view_until_the_end_or_a_cancel() {
    let canvas = answering_board();
    let core = &canvas.core;
    let mobile = view_of(core, "mobile");
    let before = board_item(core, mobile);
    drag(core, mobile, 30, -10, DragPhase::Move).expect("moves");
    // The board of the state sent now.
    assert_eq!(board_item(core, mobile).label.x, before.label.x + 30);
    let _ = crate::ipc::push::take_forced(core.host());
    drag(core, mobile, 30, -10, DragPhase::Cancel).expect("cancels");
    assert_eq!(board_item(core, mobile), before);
    assert!(crate::ipc::push::take_forced(core.host()));
    drag(core, mobile, 40, 0, DragPhase::Move).expect("moves");
    // Let go where it began: the state may be the one sent last, and still goes out.
    drag(core, mobile, 0, 0, DragPhase::End).expect("lets go");
    assert_eq!(core.host().live_moved(), None, "the trial ended");
    assert!(crate::ipc::push::take_forced(core.host()));
}

#[tokio::test]
async fn with_a_board_that_answers_a_pan_waits_for_its_frames() {
    let canvas = answering_board();
    let core = &canvas.core;
    let views = ["mobile", "tablet", "desktop"].map(|breakpoint| view_of(core, breakpoint));
    let before = views.map(|view| board_item(core, view));
    let from = canvas.ui.calls().len();
    pan(core, 50, -20, DragPhase::Move).expect("pans");
    assert_eq!(
        kinds(&canvas, from),
        ["live"],
        "the views wait for their frames"
    );
    painted(core, last_live(&canvas));
    let placed = places(&canvas, from);
    assert_eq!(placed.len(), 1);
    for item in &before {
        let place = placed[0]
            .iter()
            .find(|place| place.view == item.view)
            .expect("every view moves");
        assert_eq!(
            board_place(place, item),
            (item.label.x + 50, item.rect.y - 20)
        );
    }
    let _ = crate::ipc::push::take_forced(core.host());
    pan(core, 50, -20, DragPhase::Cancel).expect("cancels");
    assert!(crate::ipc::push::take_forced(core.host()));
    assert_eq!(core.host().live_camera(), None);
}
