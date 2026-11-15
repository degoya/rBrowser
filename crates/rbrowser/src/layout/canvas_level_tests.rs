use rb_engine::ViewHost as _;
use serde_json::{Value, json};

use super::*;
use crate::layout::canvas_live;
use crate::layout::canvas_ops::set_camera;
use crate::layout::test_support::{AREA, Setup, config, setup};

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

/// The canvas preset, every view at item zoom 0.5, the camera at 0.5: the pages at 25 %.
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

fn camera_zoom(core: &Core) -> f64 {
    canvas_live::camera(core).expect("a camera").zoom
}

fn level(answer: &Value) -> f64 {
    answer["level"].as_f64().expect("a level")
}

#[test]
fn the_steps_go_up_and_down_and_stop_at_the_ends() {
    assert!(close(step(1.0, true), 1.25));
    assert!(close(step(1.0, false), 0.75));
    assert!(close(step(0.6, true), 0.75));
    assert!(close(step(0.6, false), 0.5));
    assert!(close(step(2.0, true), 2.0));
    assert!(close(step(0.05, false), 0.025));
    assert!(close(step(0.01, false), 0.01));
    assert!(close(step(9.0, false), 2.0));
}

#[test]
fn a_step_never_goes_the_wrong_way() {
    // Review finding: below the lowest step, zooming out jumped to 5 % - a tenfold zoom-in
    // (camera 0.05 x item zoom 0.1 = 0.5 %).
    assert!(close(step(0.005, false), 0.005));
    assert!(close(step(0.03, false), 0.025));
    // Above the highest step, zooming in never zooms out.
    assert!(close(step(3.0, true), 3.0));
    for level in [0.005, 0.02, 0.3, 1.1, 2.0, 3.0] {
        assert!(step(level, true) >= level, "in from {level}");
        assert!(step(level, false) <= level, "out from {level}");
    }
}

#[test]
fn the_lowest_level_is_the_cameras_lowest_on_the_smallest_item() {
    // The status bar's slider starts there (`ui/src/statusbar/canvasZoom.ts`, MIN_PERCENT 0.5).
    assert!(close(
        MIN_LEVEL,
        canvas::ZOOM_RANGE.0 * rb_config::CANVAS_ZOOM_RANGE.0
    ));
}

#[test]
fn the_level_is_the_camera_times_the_item_zoom() {
    let camera = Camera {
        zoom: 2.0,
        ..Camera::default()
    };
    assert!(close(level_of(camera, 0.5), 1.0));
    assert!(close(camera_zoom_for(1.0, 0.5), 2.0));
    assert!(close(camera_zoom_for(1.0, 0.25), 4.0));
}

#[tokio::test]
async fn reset_shows_the_active_page_at_its_size() {
    // RB-080-94: before, the camera stopped at 1 - the page at 50 % at most.
    let canvas = board();
    let core = &canvas.core;
    assert!(close(current(core.host()).expect("a level"), 0.25));
    let answer = run(core, RESET, &Value::Null).expect("resets");
    assert!(close(level(&answer), 1.0), "{answer}");
    assert!(close(camera_zoom(core), 2.0));
    // The active view on screen is as wide as its page, in the middle of the area.
    let active = core
        .host()
        .active_view()
        .unwrap_or_else(|| core.host().views()[0].id);
    let info = core.host().view(active).expect("the view");
    let rect = info.cell.expect("a cell").rect;
    assert_eq!(rect.width, info.width);
    let middle = rect.x + i32::try_from(rect.width / 2).expect("fits");
    let area_middle = AREA.x + i32::try_from(AREA.width / 2).expect("fits");
    assert!((middle - area_middle).abs() <= 1, "{rect:?}");
}

#[tokio::test]
async fn a_level_goes_up_to_200_percent() {
    let canvas = board();
    let core = &canvas.core;
    let answer = run(core, RESET, &json!({ "level": 1.5 })).expect("zooms");
    assert!(close(level(&answer), 1.5));
    let answer = run(core, RESET, &json!({ "level": 5.0 })).expect("zooms");
    assert!(close(level(&answer), MAX_PAGE_ZOOM), "{answer}");
    assert!(run(core, RESET, &json!({ "level": -1.0 })).is_err());
    assert!(run(core, RESET, &json!({ "level": "big" })).is_err());
}

#[tokio::test]
async fn in_and_out_step_through_the_levels() {
    let canvas = board();
    let core = &canvas.core;
    run(core, RESET, &Value::Null).expect("resets");
    assert!(close(
        level(&run(core, ZOOM_IN, &Value::Null).expect("in")),
        1.25
    ));
    assert!(close(
        level(&run(core, ZOOM_OUT, &Value::Null).expect("out")),
        1.0
    ));
    assert!(close(
        level(&run(core, ZOOM_OUT, &Value::Null).expect("out")),
        0.75
    ));
}

#[tokio::test]
async fn a_place_given_stays_under_the_zoom() {
    let canvas = board();
    let core = &canvas.core;
    let before = canvas_live::camera(core).expect("a camera");
    let at = (300, 200);
    let world = (
        before.x + f64::from(at.0) / before.zoom,
        before.y + f64::from(at.1) / before.zoom,
    );
    run(core, RESET, &json!({ "x": at.0, "y": at.1 })).expect("resets");
    let after = canvas_live::camera(core).expect("a camera");
    assert!(close(after.x + f64::from(at.0) / after.zoom, world.0));
    assert!(close(after.y + f64::from(at.1) / after.zoom, world.1));
    assert!(run(core, RESET, &json!({ "x": 3 })).is_err(), "x without y");
    assert!(run(core, RESET, &json!({ "x": i32::MAX, "y": 0 })).is_err());
}

#[tokio::test]
async fn fit_shows_every_view_and_the_grid_has_no_canvas_zoom() {
    let canvas = board();
    let core = &canvas.core;
    run(core, RESET, &Value::Null).expect("resets");
    let answer = run(core, FIT, &Value::Null).expect("fits");
    assert!(level(&answer) < 1.0, "{answer}");
    assert!(camera_zoom(core) <= canvas::FIT_MAX);
    let grid = setup(config());
    assert!(run(&grid.core, RESET, &Value::Null).is_err());
    assert!(current(grid.core.host()).is_none());
}

#[tokio::test]
async fn ctrl_0_on_the_board_sets_100_percent_and_stays_the_pages_in_a_view() {
    use rb_commands::Focus;
    use rb_commands::keymap::EVENTFLAG_CONTROL_DOWN;
    let canvas = board();
    let core = &canvas.core;
    let ctrl_0 = crate::control::KeyDown {
        flags: EVENTFLAG_CONTROL_DOWN,
        windows_key_code: 0x30,
        character: Some('0'),
        unmodified_character: Some('0'),
    };
    let now = std::time::Instant::now();
    assert!(
        !core.on_key_down(Focus::View, ctrl_0, now),
        "a view's page zoom"
    );
    assert!(!core.on_key_down(Focus::Shell, ctrl_0, now));
    assert!(close(camera_zoom(core), 0.5));
    assert!(
        core.on_key_down(Focus::Board, ctrl_0, now),
        "the board's 100 %"
    );
    for _ in 0..200 {
        if close(camera_zoom(core), 2.0) {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    assert!(close(camera_zoom(core), 2.0), "{}", camera_zoom(core));
}

#[test]
fn the_reservation_lists_context_entry_names_a_command_of_the_app() {
    let defs = crate::layout::defs::definitions().expect("definitions");
    let issues = rb_commands::lint::lint_contexts(&defs, rb_commands::builtin_reserved());
    assert!(issues.is_empty(), "{issues:#?}");
}
