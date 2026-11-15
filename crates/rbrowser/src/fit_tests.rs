use super::*;

fn cell(width: u32, height: u32) -> CellRect {
    CellRect {
        x: 8,
        y: 72,
        width,
        height,
    }
}

fn emulation(width: u32, height: u32) -> Emulation {
    Emulation {
        width,
        height,
        dpr: 0.0,
        ..Emulation::default()
    }
}

#[test]
fn a_desktop_breakpoint_in_a_480_cell_keeps_its_width_and_scales_down() {
    let metrics = device_metrics(&emulation(1440, 900), cell(480, 300), Zoom::Fit);
    assert_eq!(metrics.width, 1440);
    assert!((metrics.scale - 1.0 / 3.0).abs() < 1e-9);
    assert_eq!(metrics.height, 900);
    assert_eq!(
        metrics.to_params(),
        json!({"width": 1440, "height": 900, "deviceScaleFactor": 0.0, "mobile": false, "scale": metrics.scale})
    );
}

#[test]
fn a_narrow_breakpoint_in_a_wide_cell_is_not_enlarged() {
    let metrics = device_metrics(&emulation(375, 667), cell(480, 600), Zoom::Fit);
    assert_eq!((metrics.width, metrics.height), (375, 600));
    assert!((metrics.scale - 1.0).abs() < f64::EPSILON);
}

#[test]
fn a_canvas_view_is_scaled_up_to_its_rectangle() {
    // RB-080-94: a canvas view at 200 % of its page is twice its width; the grid never enlarges.
    let canvas = device_metrics_up_to(
        &emulation(1024, 640),
        cell(2048, 1280),
        Zoom::Fit,
        MAX_CANVAS_SCALE,
    );
    assert!((canvas.scale - 2.0).abs() < 1e-9);
    assert_eq!((canvas.width, canvas.height), (1024, 640));
    // Past the canvas's largest scale the view is held to it.
    let held = device_metrics_up_to(
        &emulation(1024, 640),
        cell(4096, 2560),
        Zoom::Fit,
        MAX_CANVAS_SCALE,
    );
    assert!((held.scale - MAX_CANVAS_SCALE).abs() < 1e-9);
    let grid = device_metrics(&emulation(1024, 640), cell(2048, 1280), Zoom::Fit);
    assert!((grid.scale - 1.0).abs() < f64::EPSILON);
}

#[test]
fn a_fixed_zoom_is_clamped_and_wins_over_fit_but_not_over_an_explicit_scale() {
    let fixed = device_metrics(&emulation(1024, 768), cell(290, 368), Zoom::Factor(0.1));
    assert!((fixed.scale - MIN_ZOOM).abs() < f64::EPSILON);
    assert_eq!(fixed.height, 1472);
    let mut explicit = emulation(1024, 768);
    explicit.scale = Some(0.5);
    let metrics = device_metrics(&explicit, cell(290, 368), Zoom::Factor(0.25));
    assert!((metrics.scale - 0.5).abs() < f64::EPSILON);
}

#[test]
fn a_cell_not_laid_out_yet_keeps_the_emulated_height_and_a_valid_scale() {
    let metrics = device_metrics(&emulation(1440, 900), cell(0, 0), Zoom::Fit);
    assert_eq!(metrics.height, 900);
    assert!(metrics.scale > 0.0);
}

#[test]
fn the_display_dpr_is_kept_unless_the_emulation_sets_one() {
    let mut phone = emulation(393, 852);
    assert_eq!(device_metrics(&phone, cell(393, 852), Zoom::Fit).dpr, 0.0);
    phone.dpr = 3.0;
    phone.mobile = true;
    let metrics = device_metrics(&phone, cell(393, 852), Zoom::Fit);
    assert_eq!((metrics.dpr, metrics.mobile), (3.0, true));
}

#[test]
fn zoom_steps_move_between_the_ends() {
    assert_eq!(zoom_step(0.2, true), 0.25);
    assert_eq!(zoom_step(0.25, true), 0.33);
    assert_eq!(zoom_step(0.5, false), 0.33);
    assert_eq!(zoom_step(1.0, true), 1.0);
    assert_eq!(zoom_step(0.25, false), 0.25);
}

/// The fixture page has a 20 x 20 px button in each corner of the viewport; a click at the
/// mapped point of each button's centre must land inside the cell and come back to the button.
#[test]
fn clicks_in_a_scaled_view_map_to_the_corner_buttons() {
    let cell = cell(290, 368);
    let metrics = device_metrics(&emulation(1440, 900), cell, Zoom::Fit);
    let (width, height) = (f64::from(metrics.width), f64::from(metrics.height));
    for (cx, cy) in [
        (10.0, 10.0),
        (width - 10.0, 10.0),
        (10.0, height - 10.0),
        (width - 10.0, height - 10.0),
    ] {
        let (vx, vy) = page_to_view(cx, cy, metrics.scale);
        assert!(
            vx >= 0.0 && vx < f64::from(cell.width),
            "x {vx} outside the cell"
        );
        assert!(
            vy >= 0.0 && vy < f64::from(cell.height),
            "y {vy} outside the cell"
        );
        // Whole widget pixels, as a real click delivers them.
        let (px, py) = view_to_page(vx.round(), vy.round(), metrics.scale);
        assert!((px - cx).abs() <= 10.0, "x {px} misses the button at {cx}");
        assert!((py - cy).abs() <= 10.0, "y {py} misses the button at {cy}");
    }
}

#[test]
fn input_coordinates_are_scaled_to_widget_dip() {
    let mut mouse = json!({"type": "mouseWheel", "x": 100.0, "y": 50, "deltaX": 0, "deltaY": 120});
    scale_input("Input.dispatchMouseEvent", &mut mouse, 0.5);
    assert_eq!(
        mouse,
        json!({"type": "mouseWheel", "x": 50.0, "y": 25.0, "deltaX": 0.0, "deltaY": 60.0})
    );
    let mut touch =
        json!({"type": "touchStart", "touchPoints": [{"x": 10, "y": 20, "radiusX": 4, "id": 1}]});
    scale_input("Input.dispatchTouchEvent", &mut touch, 0.5);
    assert_eq!(
        touch["touchPoints"][0],
        json!({"x": 5.0, "y": 10.0, "radiusX": 2.0, "id": 1})
    );
}

#[test]
fn other_methods_and_scale_one_are_left_alone() {
    let original = json!({"x": 100, "y": 50});
    let mut params = original.clone();
    scale_input("Page.navigate", &mut params, 0.5);
    scale_input("Input.dispatchMouseEvent", &mut params, 1.0);
    assert_eq!(params, original);
}

#[test]
fn a_strip_reports_a_size_cef_does_not_take_for_empty() {
    // CEF's `CefSize::IsEmpty` is `width <= 0 || height <= 0`, and an empty size from a view
    // delegate falls back to the view's own (0 for a browser view): the toolbar strip's
    // `{ width: 0, height: 44 }` laid the shell out at height 0 (owner log, build 10fb32e).
    assert_eq!(strip_size(0, 44), (1, 44));
    assert_eq!(strip_size(320, 0), (320, 1));
    assert_eq!(strip_size(-5, 6), (1, 6));
    assert_eq!(strip_size(800, 600), (800, 600));
}
