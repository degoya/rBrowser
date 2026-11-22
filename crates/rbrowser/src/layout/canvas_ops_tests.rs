use serde_json::json;

use super::*;
use crate::layout::defs::{CANVAS_PLACE, CENTER, MODE_CANVAS, MODE_HORIZONTAL};
use crate::layout::test_support::{config, setup};

fn select_board(core: &Core) {
    crate::presets::run_command(core, "layout.select_preset", &json!({ "id": "board" }))
        .expect("a presets command")
        .expect("selects");
}

fn run_ok(core: &Core, id: &str, args: Value) -> Value {
    crate::layout::actions::run(core, id, &args).unwrap_or_else(|error| panic!("{id}: {error}"))
}

/// `snap: 8`: mobile moved to 13/29 snaps its right edge onto desktop's left edge (3 DIP away)
/// and its top onto the 8 DIP grid.
#[tokio::test]
async fn placing_snaps_to_a_neighbour_and_the_grid() {
    let setup = setup(config());
    let core = &setup.core;
    select_board(core);
    let spec = core.host().mode_spec().expect("canvas");
    assert_eq!(spec.breakpoints, ["desktop", "tablet", "mobile"]);
    let answer = run_ok(
        core,
        CANVAS_PLACE,
        json!({ "view": "mobile", "x": 13, "y": 29 }),
    );
    assert_eq!(
        (answer["x"].clone(), answer["y"].clone()),
        (json!(16), json!(32))
    );
    assert_eq!(answer["zoom"], json!(0.5));
    let items = core.host().shell_state().modes.fields.items;
    assert_eq!(items.len(), 3, "every view's place is kept");
    let mobile = items
        .iter()
        .find(|item| item.breakpoint == "mobile")
        .expect("mobile");
    assert_eq!((mobile.x, mobile.y), (16, 32));
}

#[tokio::test]
async fn placing_by_view_id_with_a_zoom_and_without_a_point() {
    let setup = setup(config());
    let core = &setup.core;
    select_board(core);
    let tablet = core
        .host()
        .views()
        .into_iter()
        .find(|view| view.cell_breakpoint.as_deref() == Some("tablet"))
        .expect("tablet")
        .id;
    let answer = run_ok(
        core,
        CANVAS_PLACE,
        json!({ "view": tablet.0, "zoom": 0.25 }),
    );
    assert_eq!(answer["breakpoint"], json!("tablet"));
    assert_eq!(answer["zoom"], json!(0.25));
    assert_eq!(
        answer["visible"],
        json!(true),
        "placed in the middle of the area"
    );
    let bad = crate::layout::actions::run(core, CANVAS_PLACE, &json!({ "view": "nope" }));
    assert!(matches!(bad, Err(CommandError::InvalidArgs(_))));
    let half = crate::layout::actions::run(core, CANVAS_PLACE, &json!({ "x": 4 }));
    assert!(matches!(half, Err(CommandError::InvalidArgs(_))));
}

#[tokio::test]
async fn centring_moves_the_camera_and_zoom_to_fit_resets_it() {
    let setup = setup(config());
    let core = &setup.core;
    run_ok(core, MODE_CANVAS, Value::Null);
    run_ok(core, CENTER, json!({ "x": 500, "y": 300 }));
    let camera = core.host().shell_state().modes.camera.expect("a camera");
    let shown = canvas::viewport(crate::layout::test_support::AREA, camera);
    assert!((shown.x + shown.width / 2.0 - 500.0).abs() < 1e-6);
    assert!(fit_canvas(core));
    assert_eq!(core.host().shell_state().modes.camera, None);
    run_ok(core, CENTER, Value::Null);
    assert!(core.host().shell_state().modes.camera.is_some());
}

#[tokio::test]
async fn the_strip_scrolls_only_as_far_as_it_needs() {
    let setup = setup(config());
    let core = &setup.core;
    run_ok(core, MODE_HORIZONTAL, Value::Null);
    assert_eq!(
        scroll(core, true).expect("scrolls")["first"],
        json!(0),
        "all fit"
    );
    assert_eq!(
        run_ok(core, CENTER, json!({ "index": 2 }))["first"],
        json!(0)
    );
    assert!(
        !fit_canvas(core),
        "zoom to fit is the grid's outside the canvas"
    );
}

#[tokio::test]
async fn canvas_commands_outside_the_canvas_fail_with_their_key() {
    let setup = setup(config());
    let core = &setup.core;
    match crate::layout::actions::run(core, CANVAS_PLACE, &Value::Null) {
        Err(CommandError::Failed(key)) => assert_eq!(key, "layout.error.not_canvas"),
        other => panic!("{other:?}"),
    }
    assert!(scroll(core, true).is_err());
}

/// RB-080-55, review 1: `layoutModes.showPartialViews` switched on a canvas already shown
/// places its views again at once.
#[tokio::test]
async fn the_partial_views_switch_holds_for_the_canvas_shown() {
    let setup = setup(config());
    let core = &setup.core;
    select_board(core);
    let partial = |core: &Core| match core.host().mode_spec().expect("canvas").kind {
        ModeKind::Canvas { partial, .. } => partial,
        _ => panic!("not the canvas"),
    };
    assert!(partial(core));
    let mut user = core.user_settings();
    user.layout_modes.show_partial_views = false;
    core.replace_user_settings(user.clone());
    assert!(!partial(core), "the canvas shown follows the switch");
    user.layout_modes.show_partial_views = true;
    core.replace_user_settings(user);
    assert!(partial(core));
}

/// RB-094-23: the mode shown takes a changed `layoutModes.*` at once - the minimap and the
/// space-bar pan of an open canvas follow without entering the mode again.
#[tokio::test]
async fn the_mode_shown_takes_the_changed_mode_settings_at_once() {
    let setup = setup(config());
    let core = &setup.core;
    select_board(core);
    let mut user = core.user_settings();
    user.layout_modes.minimap = !user.layout_modes.minimap;
    user.layout_modes.space_pan = !user.layout_modes.space_pan;
    core.replace_user_settings(user.clone());
    assert_eq!(
        core.host().shell_state().modes.settings,
        user.layout_modes.clamped()
    );
}
