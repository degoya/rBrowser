//! `Input.*` from outside clients, scaled to the view (ADR 0001 Decision 3 and its 2026-11-10
//! note): once a view's `scale` is below 1, CDP input coordinates are widget DIP, not CSS px.
//! A client such as Playwright computes CSS px from the page, so every coordinate, distance and
//! radius of every `Input` variant - mouse, wheel, drag, touch and the synthesized gestures - is
//! multiplied by the target's scale before it is sent.

use serde_json::Value;

/// The fields scaled per method; `touchPoints` entries take [`TOUCH_POINT`].
const FIELDS: &[(&str, &[&str])] = &[
    ("Input.dispatchMouseEvent", &["x", "y", "deltaX", "deltaY"]),
    ("Input.dispatchDragEvent", &["x", "y"]),
    (
        "Input.emulateTouchFromMouseEvent",
        &["x", "y", "deltaX", "deltaY"],
    ),
    ("Input.synthesizePinchGesture", &["x", "y"]),
    (
        "Input.synthesizeScrollGesture",
        &[
            "x",
            "y",
            "xDistance",
            "yDistance",
            "xOverscroll",
            "yOverscroll",
        ],
    ),
    ("Input.synthesizeTapGesture", &["x", "y"]),
    ("Input.dispatchTouchEvent", &[]),
];

/// The fields of one touch point.
const TOUCH_POINT: &[&str] = &["x", "y", "radiusX", "radiusY"];

/// Multiplies `method`'s coordinates in `params` by `scale` (CSS px to widget DIP). Nothing
/// changes for another method, at scale 1 or for a scale that is not a positive number.
pub fn scale_input(method: &str, params: &mut Value, scale: f64) {
    if !scale.is_finite() || scale <= 0.0 || (scale - 1.0).abs() < f64::EPSILON {
        return;
    }
    let Some((_, fields)) = FIELDS.iter().find(|(name, _)| *name == method) else {
        return;
    };
    scale_fields(params, fields, scale);
    if let Some(points) = params.get_mut("touchPoints").and_then(Value::as_array_mut) {
        for point in points {
            scale_fields(point, TOUCH_POINT, scale);
        }
    }
}

fn scale_fields(object: &mut Value, fields: &[&str], scale: f64) {
    for field in fields {
        if let Some(value) = object.get_mut(*field)
            && let Some(number) = value.as_f64()
        {
            *value = Value::from(number * scale);
        }
    }
}

#[cfg(test)]
#[path = "input_tests.rs"]
mod tests;
