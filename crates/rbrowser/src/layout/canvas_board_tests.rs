use rb_commands::CommandError;
use rb_config::MAX_CANVAS_COORD;
use rb_ipc::{DragPhase, ShellToCore};
use serde_json::json;

use super::*;
use crate::host::ui::PlacedView;
use crate::host::ui::recording::UiCall;
use crate::layout::canvas::Camera;
use crate::layout::canvas_ops::set_camera;
use crate::layout::test_support::{AREA, Setup, config, setup};

fn select_board(core: &Core) {
    crate::presets::run_command(core, "layout.select_preset", &json!({ "id": "board" }))
        .expect("a presets command")
        .expect("selects");
}

fn view_of(core: &Core, breakpoint: &str) -> ViewId {
    core.host()
        .views()
        .into_iter()
        .find(|view| view.cell_breakpoint.as_deref() == Some(breakpoint))
        .unwrap_or_else(|| panic!("{breakpoint}"))
        .id
}

/// The board with the camera at the canvas origin, zoom 1.
fn board_at_origin() -> Setup {
    let setup = setup(config());
    select_board(&setup.core);
    set_camera(
        &setup.core,
        Camera {
            x: 0.0,
            y: 0.0,
            zoom: 1.0,
        },
    );
    setup
}

fn last_place(setup: &Setup) -> Vec<PlacedView> {
    setup
        .ui
        .calls()
        .into_iter()
        .rev()
        .find_map(|call| match call {
            UiCall::Place(placed) => Some(placed),
            _ => None,
        })
        .expect("the views were placed")
}

fn placed_of(placed: &[PlacedView], view: ViewId) -> PlacedView {
    *placed
        .iter()
        .find(|place| place.view == view)
        .expect("the view is placed")
}

fn item(core: &Core, breakpoint: &str) -> (i32, i32) {
    let items = core.host().shell_state().modes.fields.items;
    let item = items
        .iter()
        .find(|item| item.breakpoint == breakpoint)
        .expect("the item");
    (item.x, item.y)
}

#[tokio::test]
async fn the_board_lists_every_view_from_the_areas_corner() {
    let setup = board_at_origin();
    let host = setup.core.host();
    let state = crate::layout::ipc::mode_state(host, &host.shell_state().modes).expect("a mode");
    let board = state.board.expect("the canvas has a board");
    assert!(board.space_pan, "layoutModes.spacePan is on by default");
    assert_eq!(board.items.len(), 3);
    let spec = host.mode_spec().expect("canvas");
    let slots = spec.arrange(&host.mode_sizes(), AREA, LABEL_HEIGHT);
    let mobile = view_of(&setup.core, "mobile");
    let index = spec
        .breakpoints
        .iter()
        .position(|id| id == "mobile")
        .expect("mobile");
    let on_board = board
        .items
        .iter()
        .find(|item| item.view == mobile)
        .expect("mobile is on the board");
    assert_eq!(on_board.label.x, slots[index].label.x - AREA.x);
    assert_eq!(on_board.label.y, slots[index].label.y - AREA.y);
    assert_eq!(on_board.label.height, LABEL_HEIGHT);
    assert_eq!(on_board.rect.y, on_board.label.y + LABEL_HEIGHT as i32);
    assert_eq!(on_board.visible, slots[index].visible);
    // The preset puts mobile at 0/20: with the camera at the origin its view starts there.
    assert_eq!((on_board.rect.x, on_board.rect.y), (0, 20));
    // The strip shows the size as the grid's strips do (RB-080-22).
    assert_eq!(on_board.text, "mobile  400 \u{d7} 800");
}

// A click on the board's label activates its view; a click on the active view's label opens
// the size quick pick under it, as in the other modes (RB-080-22).
#[tokio::test]
async fn a_second_click_on_the_active_label_opens_the_size_pick() {
    let setup = board_at_origin();
    let core = &setup.core;
    let host = core.host();
    let tablet = view_of(core, "tablet");
    assert_ne!(host.active_view(), Some(tablet));
    size_pick(core, tablet, false).expect("the first click activates");
    assert_eq!(host.active_view(), Some(tablet));
    assert!(
        !setup
            .ui
            .calls()
            .iter()
            .any(|call| matches!(call, UiCall::SizePick(..))),
        "the first click opens no pick"
    );
    size_pick(core, tablet, false).expect("the second click opens the pick");
    let state = crate::layout::ipc::mode_state(host, &host.shell_state().modes).expect("a mode");
    let label = state
        .board
        .expect("the board")
        .items
        .into_iter()
        .find(|item| item.view == tablet)
        .expect("tablet is on the board")
        .label;
    assert_eq!(
        setup.ui.calls().last(),
        Some(&UiCall::SizePick(tablet, label)),
        "the pick opens under the label the board draws"
    );
    assert!(matches!(
        size_pick(core, ViewId(99), false),
        Err(CommandError::InvalidArgs(_))
    ));
    // The view menu's "Size..." opens the pick without activating (RB-080-41).
    let mobile = view_of(core, "mobile");
    size_pick(core, mobile, true).expect("forced");
    assert_eq!(host.active_view(), Some(tablet), "nothing activated");
    assert!(matches!(
        setup.ui.calls().last(),
        Some(UiCall::SizePick(view, _)) if *view == mobile
    ));
}

/// The board gets the camera, so a click on the free area becomes a canvas point (RB-080-41).
#[test]
fn the_board_camera_turns_a_click_into_a_canvas_point() {
    let camera = board_camera(Camera {
        x: -40.4,
        y: 12.0,
        zoom: 0.5,
    });
    assert_eq!(
        camera,
        rb_ipc::BoardCamera {
            x: -40,
            y: 12,
            zoom_permille: 500
        }
    );
    assert_eq!(board_point(camera, 100, 50), (160.0, 112.0));
    let wild = board_camera(Camera {
        x: f64::NAN,
        y: 0.0,
        zoom: 0.0,
    });
    assert_eq!(wild.x, 0);
    assert!(wild.zoom_permille >= 1);
}

#[tokio::test]
async fn the_board_is_the_canvas_only() {
    let setup = setup(config());
    let core = &setup.core;
    crate::presets::run_command(core, "layout.select_preset", &json!({ "id": "pair" }))
        .expect("a presets command")
        .expect("selects");
    let host = core.host();
    let state = crate::layout::ipc::mode_state(host, &host.shell_state().modes).expect("a mode");
    assert!(state.board.is_none());
    assert!(matches!(
        drag(core, view_of(core, "mobile"), 1, 1, DragPhase::Move),
        Err(CommandError::Failed(_))
    ));
    assert!(matches!(
        size_pick(core, view_of(core, "mobile"), false),
        Err(CommandError::Failed(_))
    ));
}

#[tokio::test]
async fn a_drag_moves_the_view_live_and_keeps_nothing() {
    let setup = board_at_origin();
    let core = &setup.core;
    let mobile = view_of(core, "mobile");
    let before = placed_of(&last_place(&setup), mobile);
    let items = core.host().shell_state().modes.fields.items;
    drag(core, mobile, 30, -10, DragPhase::Move).expect("moves");
    let placed = last_place(&setup);
    assert_eq!(placed.len(), 1, "only the dragged view moves");
    let now = placed_of(&placed, mobile);
    assert_eq!(
        (now.outer.x, now.outer.y),
        (before.outer.x + 30, before.outer.y - 10)
    );
    assert_eq!(now.outer.width, before.outer.width);
    assert_eq!(
        core.host().shell_state().modes.fields.items,
        items,
        "nothing is kept while dragging"
    );
}

#[tokio::test]
async fn letting_go_snaps_and_keeps_the_place() {
    let setup = board_at_origin();
    let core = &setup.core;
    let mobile = view_of(core, "mobile");
    drag(core, mobile, -150, -100, DragPhase::Move).expect("moves");
    // From 0/20 by -301/-203: -301/-183 on the 8 DIP grid, far from every other view.
    drag(core, mobile, -301, -203, DragPhase::End).expect("lets go");
    assert_eq!(item(core, "mobile"), (-304, -184));
    let placed = placed_of(&last_place(&setup), mobile);
    assert_eq!(placed.outer.x, AREA.x - 304);
}

#[tokio::test]
async fn the_cameras_zoom_scales_the_drag() {
    let setup = setup(config());
    let core = &setup.core;
    select_board(core);
    set_camera(
        core,
        Camera {
            x: 0.0,
            y: 0.0,
            zoom: 0.5,
        },
    );
    let mobile = view_of(core, "mobile");
    // 100 screen DIP at zoom 0.5 are 200 canvas DIP; the top stays on desktop's.
    drag(core, mobile, -100, 0, DragPhase::End).expect("lets go");
    assert_eq!(item(core, "mobile"), (-200, 20));
}

#[tokio::test]
async fn escape_puts_the_view_back() {
    let setup = board_at_origin();
    let core = &setup.core;
    let mobile = view_of(core, "mobile");
    let before = placed_of(&last_place(&setup), mobile);
    drag(core, mobile, 120, 40, DragPhase::Move).expect("moves");
    drag(core, mobile, 120, 40, DragPhase::Cancel).expect("cancels");
    assert_eq!(placed_of(&last_place(&setup), mobile).outer, before.outer);
    assert_eq!(item(core, "mobile"), (0, 20));
}

#[tokio::test]
async fn a_view_is_never_dragged_off_the_canvas() {
    let setup = setup(config());
    let core = &setup.core;
    select_board(core);
    set_camera(
        core,
        Camera {
            x: 0.0,
            y: 0.0,
            zoom: 0.05,
        },
    );
    let mobile = view_of(core, "mobile");
    drag(core, mobile, MAX_DRAG, -MAX_DRAG, DragPhase::End).expect("lets go");
    assert_eq!(item(core, "mobile"), (MAX_CANVAS_COORD, -MAX_CANVAS_COORD));
}

#[tokio::test]
async fn a_pan_moves_every_view_and_letting_go_keeps_the_camera() {
    let setup = board_at_origin();
    let core = &setup.core;
    let before = last_place(&setup);
    pan(core, 50, -20, DragPhase::Move).expect("pans");
    let placed = last_place(&setup);
    assert_eq!(placed.len(), 3);
    for place in &before {
        let now = placed_of(&placed, place.view);
        assert_eq!(
            (now.outer.x, now.outer.y),
            (place.outer.x + 50, place.outer.y - 20)
        );
    }
    let camera = core.host().shell_state().modes.camera.expect("a camera");
    assert_eq!(
        (camera.x, camera.y),
        (0.0, 0.0),
        "nothing kept while panning"
    );
    pan(core, 50, -20, DragPhase::End).expect("lets go");
    let camera = core.host().shell_state().modes.camera.expect("a camera");
    assert_eq!((camera.x, camera.y, camera.zoom), (-50.0, 20.0, 1.0));
    pan(core, 10, 10, DragPhase::Cancel).expect("cancels");
    let camera = core.host().shell_state().modes.camera.expect("a camera");
    assert_eq!((camera.x, camera.y), (-50.0, 20.0));
}

#[tokio::test]
async fn an_arrow_key_moves_to_the_next_snap_line() {
    let setup = board_at_origin();
    let core = &setup.core;
    let mobile = view_of(core, "mobile");
    step(core, mobile, 1, 0).expect("steps");
    assert_eq!(item(core, "mobile"), (8, 20));
    step(core, mobile, 0, -1).expect("steps");
    assert_eq!(item(core, "mobile"), (8, 16));
    step(core, mobile, -1, -1).expect("steps");
    assert_eq!(item(core, "mobile"), (0, 8));
}

#[tokio::test]
async fn wild_input_is_refused() {
    let setup = board_at_origin();
    let core = &setup.core;
    let mobile = view_of(core, "mobile");
    assert!(matches!(
        drag(core, mobile, MAX_DRAG + 1, 0, DragPhase::Move),
        Err(CommandError::InvalidArgs(_))
    ));
    assert!(matches!(
        pan(core, 0, i32::MIN, DragPhase::End),
        Err(CommandError::InvalidArgs(_))
    ));
    assert!(matches!(
        step(core, mobile, 2, 0),
        Err(CommandError::InvalidArgs(_))
    ));
    assert!(matches!(
        drag(core, ViewId(999), 1, 1, DragPhase::Move),
        Err(CommandError::InvalidArgs(_))
    ));
    assert_eq!(item(core, "mobile"), (0, 20));
}

#[tokio::test]
async fn the_board_requests_are_answered_through_the_shell_ipc() {
    let setup = board_at_origin();
    let core = &setup.core;
    let mobile = view_of(core, "mobile");
    let request = serde_json::to_string(&ShellToCore::CanvasStep {
        view: mobile,
        dx: 1,
        dy: 0,
    })
    .expect("encodes");
    let answer = crate::ipc::dispatch(core, &request).expect("answered");
    assert!(answer.contains("\"ack\""), "{answer}");
    assert_eq!(item(core, "mobile"), (8, 20));
    let pick = serde_json::to_string(&ShellToCore::CanvasSizePick {
        view: mobile,
        force: false,
    })
    .expect("encodes");
    let answer = crate::ipc::dispatch(core, &pick).expect("answered");
    assert!(answer.contains("\"ack\""), "{answer}");
    let refused = serde_json::to_string(&ShellToCore::CanvasPan {
        dx: MAX_DRAG + 1,
        dy: 0,
        phase: DragPhase::Move,
    })
    .expect("encodes");
    let (code, _) = crate::ipc::dispatch(core, &refused).expect_err("refused");
    assert_eq!(code, rb_ipc::IpcErrorCode::BadRequest);
}

#[test]
fn an_overlay_leaves_the_label_strip_to_the_board() {
    let outer = CellRect {
        x: 10,
        y: 64,
        width: 200,
        height: 420,
    };
    assert_eq!(
        below_label(outer, LABEL_HEIGHT),
        CellRect {
            x: 10,
            y: 84,
            width: 200,
            height: 400,
        }
    );
    assert_eq!(below_label(outer, 0), outer);
    assert_eq!(below_label(outer, 1000).height, 0);
}

#[test]
fn the_pure_steps() {
    let camera = Camera {
        x: 0.0,
        y: 0.0,
        zoom: 0.25,
    };
    assert_eq!(moved((10, 20), (5, -5), camera), (30.0, 0.0));
    assert_eq!(
        moved((0, 0), (MAX_DRAG, -MAX_DRAG), camera),
        (f64::from(MAX_CANVAS_COORD), -f64::from(MAX_CANVAS_COORD))
    );
    let panned = panned(camera, (10, -4));
    assert_eq!((panned.x, panned.y, panned.zoom), (-40.0, 16.0, 0.25));
    assert_eq!(stepped(20, 0, 8), 20);
    assert_eq!(stepped(20, 1, 8), 32, "the next line one step on");
    assert_eq!(stepped(16, -1, 8), 8);
    assert_eq!(stepped(5, 1, 0), 6, "1 DIP without a grid");
    assert_eq!(stepped(MAX_CANVAS_COORD, 1, 8), MAX_CANVAS_COORD);
}

#[tokio::test]
async fn a_click_on_the_boards_engine_badge_opens_the_engine_menu_under_the_label() {
    let setup = board_at_origin();
    let core = &setup.core;
    let mobile = view_of(core, "mobile");
    let desktop = view_of(core, "desktop");
    core.host().set_active_view(desktop).expect("activates");
    engine_pick(core, mobile).expect("the view is on the canvas");
    let host = core.host();
    let state = crate::layout::ipc::mode_state(host, &host.shell_state().modes).expect("a mode");
    let label = state
        .board
        .expect("the canvas has a board")
        .items
        .into_iter()
        .find(|item| item.view == mobile)
        .map(|item| item.label)
        .expect("the view is drawn");
    assert_eq!(
        setup.ui.calls().last(),
        Some(&UiCall::EnginePick(mobile, Some(label)))
    );
    assert_eq!(
        core.host().active_view(),
        Some(desktop),
        "the badge activates nothing"
    );
    assert!(matches!(
        engine_pick(core, ViewId(999)),
        Err(CommandError::InvalidArgs(_))
    ));
    let pick =
        serde_json::to_string(&ShellToCore::CanvasEnginePick { view: mobile }).expect("encodes");
    let answer = crate::ipc::dispatch(core, &pick).expect("answered");
    assert!(answer.contains("\"ack\""), "{answer}");
}
