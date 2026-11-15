use serde_json::json;

use super::*;

#[test]
fn mouse_coordinates_and_wheel_deltas_are_scaled() {
    let mut params = json!({"type": "mouseWheel", "x": 100, "y": 50.5, "deltaX": 10, "deltaY": -20, "button": "none", "clickCount": 1});
    scale_input("Input.dispatchMouseEvent", &mut params, 0.5);
    assert_eq!(params["x"], json!(50.0));
    assert_eq!(params["y"], json!(25.25));
    assert_eq!(params["deltaX"], json!(5.0));
    assert_eq!(params["deltaY"], json!(-10.0));
    assert_eq!(params["clickCount"], json!(1));
}

#[test]
fn touch_points_drag_and_gestures_are_scaled() {
    let mut touch = json!({"type": "touchStart", "touchPoints": [{"x": 10, "y": 20, "radiusX": 4, "radiusY": 2, "id": 7}]});
    scale_input("Input.dispatchTouchEvent", &mut touch, 0.5);
    assert_eq!(
        touch["touchPoints"][0],
        json!({"x": 5.0, "y": 10.0, "radiusX": 2.0, "radiusY": 1.0, "id": 7})
    );

    let mut drag = json!({"type": "drop", "x": 8, "y": 4, "data": {"items": []}});
    scale_input("Input.dispatchDragEvent", &mut drag, 0.25);
    assert_eq!(
        (drag["x"].clone(), drag["y"].clone()),
        (json!(2.0), json!(1.0))
    );

    let mut scroll = json!({"x": 100, "y": 100, "xDistance": -40, "yDistance": 80, "xOverscroll": 0, "yOverscroll": 10, "speed": 800});
    scale_input("Input.synthesizeScrollGesture", &mut scroll, 0.5);
    assert_eq!(scroll["yDistance"], json!(40.0));
    assert_eq!(scroll["yOverscroll"], json!(5.0));
    assert_eq!(scroll["speed"], json!(800));

    for method in [
        "Input.emulateTouchFromMouseEvent",
        "Input.synthesizePinchGesture",
        "Input.synthesizeTapGesture",
    ] {
        let mut params = json!({"x": 2, "y": 4});
        scale_input(method, &mut params, 0.5);
        assert_eq!(params, json!({"x": 1.0, "y": 2.0}), "{method}");
    }
}

#[test]
fn nothing_changes_at_scale_one_for_other_methods_or_bad_scales() {
    let original = json!({"x": 3, "y": 4});
    for (method, scale) in [
        ("Input.dispatchMouseEvent", 1.0),
        ("Page.navigate", 0.5),
        ("Input.insertText", 0.5),
        ("Input.dispatchKeyEvent", 0.5),
        ("Input.dispatchMouseEvent", 0.0),
        ("Input.dispatchMouseEvent", f64::NAN),
        ("Input.dispatchMouseEvent", -1.0),
    ] {
        let mut params = original.clone();
        scale_input(method, &mut params, scale);
        assert_eq!(params, original, "{method} {scale}");
    }
}

#[test]
fn a_field_that_is_no_number_is_left_alone() {
    let mut params = json!({"x": "10", "y": null});
    scale_input("Input.dispatchMouseEvent", &mut params, 0.5);
    assert_eq!(params, json!({"x": "10", "y": null}));
}
