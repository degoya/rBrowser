//! RB-080-54: a view whose size changes in a layout mode is placed again at once - the window
//! gets its new rectangle with the change, not with the next drag.

use rb_config::{EmulationFields, Orientation};
use rb_engine::{ViewHost as _, ViewId};
use serde_json::json;

use crate::control::Core;
use crate::host::ui::PlacedView;
use crate::host::ui::recording::UiCall;
use crate::layout::canvas::Camera;
use crate::layout::canvas_ops::set_camera;
use crate::layout::test_support::{Setup, config, setup};

fn view_of(core: &Core, breakpoint: &str) -> ViewId {
    core.host()
        .views()
        .into_iter()
        .find(|view| view.cell_breakpoint.as_deref() == Some(breakpoint))
        .unwrap_or_else(|| panic!("{breakpoint}"))
        .id
}

/// The preset `id` shown; the canvas with its camera at the origin, zoom 1.
fn shown(id: &str) -> Setup {
    let setup = setup(config());
    crate::presets::run_command(&setup.core, "layout.select_preset", &json!({ "id": id }))
        .expect("a presets command")
        .expect("selects");
    if id == "board" {
        let origin = Camera {
            x: 0.0,
            y: 0.0,
            zoom: 1.0,
        };
        set_camera(&setup.core, origin);
    }
    setup
}

/// How often the window was told where the views go.
fn places(setup: &Setup) -> usize {
    setup
        .ui
        .calls()
        .iter()
        .filter(|call| matches!(call, UiCall::Place(_)))
        .count()
}

/// Where the window was last told to put `view`.
fn place_of(setup: &Setup, view: ViewId) -> PlacedView {
    setup
        .ui
        .calls()
        .into_iter()
        .rev()
        .find_map(|call| match call {
            UiCall::Place(placed) => placed.into_iter().find(|place| place.view == view),
            _ => None,
        })
        .expect("the view was placed")
}

fn size(place: PlacedView) -> (u32, u32) {
    (place.outer.width, place.outer.height)
}

// The preset shows mobile (400 x 800) at zoom 0.5 under its 20 DIP label.
#[tokio::test]
async fn a_free_size_on_the_canvas_places_the_view_at_once() {
    let setup = shown("board");
    let host = setup.core.host();
    let mobile = view_of(&setup.core, "mobile");
    assert_eq!(size(place_of(&setup, mobile)), (200, 420));
    let before = places(&setup);

    host.set_size(mobile, 600, 1000, None).expect("a size");

    assert!(places(&setup) > before, "the window places the views again");
    assert_eq!(size(place_of(&setup, mobile)), (300, 520));
}

#[tokio::test]
async fn a_breakpoint_on_the_canvas_places_the_view_at_once() {
    let setup = shown("board");
    let host = setup.core.host();
    let mobile = view_of(&setup.core, "mobile");
    let tablet = host
        .config()
        .breakpoints
        .into_iter()
        .find(|breakpoint| breakpoint.id == "tablet")
        .expect("tablet");
    host.set_breakpoint(mobile, &tablet).expect("a breakpoint");
    assert_eq!(size(place_of(&setup, mobile)), (384, 532));
}

// The orientation swaps width and height (`emulation.set`, a device preset).
#[tokio::test]
async fn a_turned_view_on_the_canvas_is_placed_at_once_and_a_colour_scheme_places_nothing() {
    let setup = shown("board");
    let host = setup.core.host();
    let mobile = view_of(&setup.core, "mobile");
    let before = places(&setup);
    let dark = EmulationFields {
        color_scheme: Some(rb_config::ColorScheme::Dark),
        ..EmulationFields::default()
    };
    host.set_emulation_fields(mobile, &dark).expect("dark");
    assert_eq!(places(&setup), before, "the size did not change");

    let turned = EmulationFields {
        orientation: Some(Orientation::Landscape),
        ..EmulationFields::default()
    };
    host.set_emulation_fields(mobile, &turned).expect("turned");
    assert_eq!(size(place_of(&setup, mobile)), (400, 220));
    host.reset_emulation(mobile).expect("back");
    assert_eq!(size(place_of(&setup, mobile)), (200, 420));
}

// The split's sides do not depend on a view's size: nothing moves, nothing is placed.
#[tokio::test]
async fn a_size_in_grid_mode_places_nothing() {
    let setup = setup(config());
    let host = setup.core.host();
    let mobile = view_of(&setup.core, "mobile");
    host.set_size(mobile, 600, 1000, None).expect("a size");
    assert_eq!(places(&setup), 0, "the grid lays its cells out itself");
}
