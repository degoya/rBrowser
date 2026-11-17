use rb_config::Zoom;
use rb_engine::{CellRect, Emulation};
use rb_sync::{MOD_SHIFT, MouseButton};

use super::*;
use crate::fit::{device_metrics, scale_input, view_to_page};

fn methods(calls: &[Call]) -> Vec<String> {
    calls
        .iter()
        .map(|call| {
            let kind = call.params["type"].as_str().unwrap_or_default();
            format!("{}:{kind}", call.method)
        })
        .collect()
}

#[test]
fn without_pinch_zoom_the_resolved_point_is_the_input_point() {
    assert_eq!(input_point(120.5, 40.0, None), (120.5, 40.0));
    assert_eq!(
        input_point(120.5, 40.0, Some(VisualViewport::IDENTITY)),
        (120.5, 40.0)
    );
}

#[test]
fn a_pinch_zoomed_visual_viewport_moves_and_scales_the_point() {
    // The agent's point is in the layout viewport; CDP input is relative to the visual one.
    let viewport = VisualViewport {
        offset_left: 10.0,
        offset_top: 30.0,
        scale: 2.0,
    };
    assert_eq!(input_point(110.0, 80.0, Some(viewport)), (200.0, 100.0));
}

#[test]
fn a_visual_viewport_that_makes_no_sense_is_ignored() {
    for viewport in [
        VisualViewport {
            offset_left: f64::NAN,
            offset_top: 0.0,
            scale: 1.0,
        },
        VisualViewport {
            offset_left: 0.0,
            offset_top: 0.0,
            scale: 0.0,
        },
        VisualViewport {
            offset_left: 0.0,
            offset_top: 0.0,
            scale: f64::INFINITY,
        },
    ] {
        assert_eq!(input_point(5.0, 6.0, Some(viewport)), (5.0, 6.0));
    }
}

#[test]
fn page_scroll_is_already_in_the_resolved_point() {
    // The agent answers client coordinates of its viewport: a scrolled page changes nothing
    // here, only a scrolled visual viewport (pinch zoom) does.
    let scrolled = VisualViewport {
        offset_left: 0.0,
        offset_top: 250.0,
        scale: 1.0,
    };
    assert_eq!(input_point(50.0, 300.0, Some(scrolled)), (50.0, 50.0));
}

#[test]
fn a_click_in_a_view_at_scale_0_33_hits_a_20_px_target() {
    // A 20 x 20 px button at (400, 300) in a 1280 px breakpoint shown in a 422 px cell.
    let emulation = Emulation {
        width: 1280,
        ..Emulation::default()
    };
    let cell = CellRect {
        x: 0,
        y: 0,
        width: 422,
        height: 600,
    };
    let scale = device_metrics(&emulation, cell, Zoom::Fit).scale;
    assert!((scale - 0.33).abs() < 0.01, "scale {scale}");
    let (x, y) = input_point(410.0, 310.0, None);
    for call in click_calls(x, y, MouseButton::Left, 1, 0) {
        let mut params = call.params;
        scale_input(call.method, &mut params, scale);
        let (dip_x, dip_y) = (params["x"].as_f64(), params["y"].as_f64());
        let (dip_x, dip_y) = (dip_x.expect("x"), dip_y.expect("y"));
        // What the page gets back from the widget DIP the view receives.
        let (page_x, page_y) = view_to_page(dip_x, dip_y, scale);
        assert!((400.0..420.0).contains(&page_x), "{page_x}");
        assert!((300.0..320.0).contains(&page_y), "{page_y}");
    }
}

#[test]
fn the_device_pixel_ratio_does_not_move_the_point() {
    // DPR scales device pixels only; CDP input is in DIP, which the scale alone sets.
    let cell = CellRect {
        x: 0,
        y: 0,
        width: 393,
        height: 852,
    };
    let at = |dpr: f64| {
        let emulation = Emulation {
            width: 393,
            dpr,
            ..Emulation::default()
        };
        let scale = device_metrics(&emulation, cell, Zoom::Fit).scale;
        let mut params = click_calls(100.0, 200.0, MouseButton::Left, 1, 0)[1]
            .params
            .clone();
        scale_input("Input.dispatchMouseEvent", &mut params, scale);
        (params["x"].as_f64(), params["y"].as_f64())
    };
    assert_eq!(at(1.0), at(3.0));
    assert_eq!(at(3.0), (Some(100.0), Some(200.0)));
}

#[test]
fn a_click_is_move_press_release_with_its_count_and_buttons() {
    let calls = click_calls(10.0, 20.0, MouseButton::Left, 1, MOD_SHIFT);
    assert_eq!(
        methods(&calls),
        [
            "Input.dispatchMouseEvent:mouseMoved",
            "Input.dispatchMouseEvent:mousePressed",
            "Input.dispatchMouseEvent:mouseReleased",
        ]
    );
    let (moved, pressed, released) = (&calls[0].params, &calls[1].params, &calls[2].params);
    assert_eq!(moved["button"], "none");
    assert_eq!(moved["buttons"], 0);
    assert_eq!(pressed["button"], "left");
    assert_eq!(pressed["buttons"], 1);
    assert_eq!(pressed["clickCount"], 1);
    assert_eq!(pressed["modifiers"], u64::from(MOD_SHIFT));
    assert_eq!(released["buttons"], 0);
    assert_eq!(released["clickCount"], 1);
    for params in [moved, pressed, released] {
        assert_eq!(
            (params["x"].as_f64(), params["y"].as_f64()),
            (Some(10.0), Some(20.0))
        );
        assert_eq!(params["pointerType"], "mouse");
    }
}

#[test]
fn the_second_click_of_a_double_click_counts_two() {
    let calls = click_calls(1.0, 2.0, MouseButton::Left, 2, 0);
    assert_eq!(calls[1].params["clickCount"], 2);
    assert_eq!(calls[2].params["clickCount"], 2);
    // A count past three is the same as three (a triple click selects the paragraph).
    assert_eq!(
        click_calls(1.0, 2.0, MouseButton::Left, 9, 0)[1].params["clickCount"],
        3
    );
}

#[test]
fn each_button_has_its_bit() {
    assert_eq!(button_mask(MouseButton::None), 0);
    assert_eq!(button_mask(MouseButton::Left), 1);
    assert_eq!(button_mask(MouseButton::Right), 2);
    assert_eq!(button_mask(MouseButton::Middle), 4);
    assert_eq!(button_mask(MouseButton::Back), 8);
    assert_eq!(button_mask(MouseButton::Forward), 16);
    let right = click_calls(0.0, 0.0, MouseButton::Right, 1, 0);
    assert_eq!(right[1].params["button"], "right");
    assert_eq!(right[1].params["buttons"], 2);
}

#[test]
fn hover_is_one_mouse_move_without_buttons() {
    let calls = hover_calls(3.0, 4.0);
    assert_eq!(methods(&calls), ["Input.dispatchMouseEvent:mouseMoved"]);
    assert_eq!(calls[0].params["buttons"], 0);
}

#[test]
fn a_tap_is_touch_start_and_end() {
    let calls = tap_calls(30.0, 40.0, 0);
    assert_eq!(
        methods(&calls),
        [
            "Input.dispatchTouchEvent:touchStart",
            "Input.dispatchTouchEvent:touchEnd",
        ]
    );
    let point = &calls[0].params["touchPoints"][0];
    assert_eq!(
        (point["x"].as_f64(), point["y"].as_f64()),
        (Some(30.0), Some(40.0))
    );
    assert_eq!(
        calls[1].params["touchPoints"].as_array().map(Vec::len),
        Some(0)
    );
}

#[test]
fn inserted_text_never_shows_in_debug_output() {
    let call = insert_text_call("hunter2");
    assert_eq!(call.method, "Input.insertText");
    assert_eq!(call.params["text"], "hunter2");
    assert!(!format!("{call:?}").contains("hunter2"));
}
