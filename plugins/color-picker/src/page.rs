//! The page script's requests (`call-tool("@page", {"view", "message"})`, docs/plugins.md "Page
//! scripts and panels"):
//!
//! - `{"type": "config"}`: the settings, which the page script cannot read itself;
//! - `{"type": "calibrate", "x", "y", "width", "height", "dpr", "scrollX", "scrollY", "marker",
//!   "pointed"?}`: where the viewport lies in the view's window image, the page script showing
//!   its markers at `marker` (two to six `[x, y]`, their top left corners in CSS px of the
//!   viewport; `pointed: true` where two of them lie beside the person's pointer, RB-090-40) -
//!   `{"map": {"left", "top", "scale", "width", "height"}, "exact", "visible": {"left", "top",
//!   "right", "bottom"}}` (`left`, `top` below 0 where the window's edge cut the view; `visible`
//!   the part of the viewport in CSS px the image is known to show, where the loupe goes -
//!   [`crate::visible`]), or `{"map": null, "reason": "image" | "size" |
//!   "markers"}` where the image does not map it ([`crate::shot`], RB-080-77, RB-090-19), once
//!   per pipette and view;
//! - `{"type": "tile", "x", "y", "width", "height", "dpr", "scrollX", "scrollY", "map" | "clip"}`:
//!   the pixels around a viewport point where the pointer rests - the request of the moving
//!   pipette that takes a screenshot; the page script draws the loupe from it
//!   ([`crate::sample::tile`], RB-080-56); `width`, `height` are the viewport's in CSS px; `map`
//!   as `calibrate` answered it, or `clip: true` where the settings ask for clips;
//! - `{"type": "unreadable", "reason"?}`: the window image of the view did not map it, three
//!   times - the status bar says so and why (`reason` as `calibrate` answered it; the pipette
//!   reads nothing there in this pipette);
//! - `{"type": "value", "color": "#rrggbb"}`: that colour in the format of the settings, for the
//!   loupe's label - no screenshot;
//! - `{"type": "take", ..., "slot": "foreground" | "background", "exact"?}`: the person took the
//!   colour there - posted to the panel (with `managed` where the window image showed the
//!   display's colours, `exact: false`) and the status bar, and every view's pipette stops;
//! - `{"type": "stopped"}`: the person ended the pipette by Esc - every view's pipette stops, the
//!   panel and the status bar say so (RB-080-64);
//! - `{"type": "chosen", "selector"}`: the person chose an element to measure; it is measured in
//!   every view ([`crate::measure`]).

use rb_plugin_api::{Failure, FailureKind, PluginHost, ViewId};
use serde_json::{Value, json};

use crate::convert::Formats;
use crate::sample::{
    MAX_COORDINATE, Map, Marker, Read, Viewport, capture, coordinate, tile, to_json,
};
use crate::settings::Settings;
use crate::shot::{Unmapped, calibrate_view};
use crate::{color, failure, invalid, measure, notice, stopped, targets};

fn number(value: &Value) -> Option<f64> {
    value.as_f64().filter(|number| number.is_finite())
}

/// The largest image side a map may name: 16 times the largest coordinate, like a DPR of 16.
const MAX_IMAGE_SIDE: f64 = 1_600_000.0;

/// The `marker` of a `calibrate` message: two to six points (four corners, two beside the pointer).
fn marker_of(message: &Value) -> Result<Marker, Failure> {
    match &message["marker"] {
        Value::Array(points) if (2..=6).contains(&points.len()) => points
            .iter()
            .map(|at| Some((coordinate(&at[0])?, coordinate(&at[1])?)))
            .collect::<Option<Marker>>()
            .ok_or_else(|| invalid("marker")),
        _ => Err(invalid("marker")),
    }
}

/// The `map` of a `tile` or `take` message, as `calibrate` answered it: its offset below 0 where
/// the window's edge cut the view (RB-090-40).
fn map_of(map: &Value) -> Result<Map, Failure> {
    let whole = |key: &str, low: f64| {
        number(&map[key])
            .filter(|value| value.fract() == 0.0 && (low..=MAX_IMAGE_SIDE).contains(value))
            .ok_or_else(|| invalid("map"))
    };
    let scale = number(&map["scale"])
        .filter(|scale| *scale > 0.0 && *scale <= 16.0)
        .ok_or_else(|| invalid("map"))?;
    // Whole numbers inside the bounds above: exact as integers.
    Ok(Map {
        left: whole("left", -MAX_IMAGE_SIDE)? as i64,
        top: whole("top", -MAX_IMAGE_SIDE)? as i64,
        scale,
        width: whole("width", 1.0)? as u32,
        height: whole("height", 1.0)? as u32,
    })
}

/// How a `tile` or `take` message asks for the view to be read: by its `map` or a `clip`.
fn read_of(message: &Value) -> Result<Read, Failure> {
    match (&message["map"], &message["clip"]) {
        (Value::Null, Value::Bool(true)) => Ok(Read::Clip),
        (map @ Value::Object(_), Value::Null | Value::Bool(false)) => {
            Ok(Read::Window(map_of(map)?))
        }
        (_, Value::Null | Value::Bool(_)) => Err(invalid("map")),
        _ => Err(invalid("clip")),
    }
}

/// Refuses a request of a view the plugin does not read (a Firefox or WebKit view), before any
/// CDP call.
fn chromium(host: &mut dyn PluginHost, view: ViewId) -> Result<(), Failure> {
    let views = host.views()?;
    match views.iter().find(|info| info.id == view) {
        Some(info) if targets::supported(info) => Ok(()),
        Some(_) => Err(failure(FailureKind::NotSupported, "unsupported_engine")),
        None => {
            Err(failure(FailureKind::NotFound, "unknown_view").with_param("view", view.to_string()))
        }
    }
}

/// The status bar's word that `view` could not be read without flicker, and why: `reason` as
/// `calibrate` answered it.
fn unreadable(host: &mut dyn PluginHost, view: ViewId, reason: Option<&str>) -> Value {
    let name = if reason == Some(Unmapped::Size.word()) {
        "unreadable_size"
    } else if reason == Some(Unmapped::Image.word()) {
        "unreadable_image"
    } else {
        "unreadable"
    };
    let label = host
        .views()
        .ok()
        .and_then(|views| views.into_iter().find(|info| info.id == view))
        .map_or_else(|| view.to_string(), |info| info.label);
    json!({ "$post": [notice(name, "info", &json!({ "view": label }))] })
}

/// The viewport point and the viewport of a `tile` or `take` message.
fn point(message: &Value) -> Result<((f64, f64), Viewport), Failure> {
    let x = coordinate(&message["x"]).ok_or_else(|| invalid("x"))?;
    let y = coordinate(&message["y"]).ok_or_else(|| invalid("y"))?;
    let size = |key: &str| {
        number(&message[key])
            .filter(|size| (1.0..=MAX_COORDINATE).contains(size))
            .ok_or_else(|| invalid(key))
    };
    let dpr = number(&message["dpr"])
        .filter(|dpr| (0.25..=8.0).contains(dpr))
        .ok_or_else(|| invalid("dpr"))?;
    // An overscrolled page reports a negative offset for a moment: the document's edge.
    let scroll = |key: &str| {
        number(&message[key])
            .map(|offset| offset.clamp(0.0, MAX_COORDINATE))
            .ok_or_else(|| invalid(key))
    };
    let port = Viewport {
        width: size("width")?,
        height: size("height")?,
        dpr,
        scroll_x: scroll("scrollX")?,
        scroll_y: scroll("scrollY")?,
    };
    Ok(((x, y), port))
}

/// Answers one page request.
///
/// # Errors
///
/// `invalid` for a message that does not read; the CDP or evaluate failure.
pub fn handle(host: &mut dyn PluginHost, args: &Value) -> Result<Value, Failure> {
    let view = args["view"]
        .as_u64()
        .and_then(|view| ViewId::try_from(view).ok())
        .ok_or_else(|| invalid("view"))?;
    let message = &args["message"];
    match message["type"].as_str() {
        Some("config") => Ok(Settings::read(host).to_json()),
        Some("calibrate") => {
            chromium(host, view)?;
            let (_, port) = point(message)?;
            let pointed = message["pointed"] == Value::Bool(true);
            match calibrate_view(host, view, &port, &marker_of(message)?, pointed) {
                Ok((map, exact, seen)) => Ok(json!({
                    "map": {
                        "left": map.left,
                        "top": map.top,
                        "scale": map.scale,
                        "width": map.width,
                        "height": map.height,
                    },
                    "exact": exact,
                    "visible": {
                        "left": seen.left,
                        "top": seen.top,
                        "right": seen.right,
                        "bottom": seen.bottom,
                    },
                })),
                Err(why) => Ok(json!({ "map": null, "reason": why.word() })),
            }
        }
        Some("tile") => {
            chromium(host, view)?;
            let (at, port) = point(message)?;
            tile(host, view, &port, at, &read_of(message)?)
        }
        Some("unreadable") => Ok(unreadable(host, view, message["reason"].as_str())),
        Some("value") => {
            let color = message["color"]
                .as_str()
                .filter(|text| text.len() == 7 && text.starts_with('#'))
                .and_then(color::parse)
                .ok_or_else(|| invalid("color"))?;
            let format = Settings::read(host).format;
            Ok(json!({ "value": Formats::of(color).get(format) }))
        }
        Some("take") => {
            let settings = Settings::read(host);
            let slot = match message["slot"].as_str() {
                Some("background") => "background",
                Some("foreground") | None => "foreground",
                Some(_) => return Err(invalid("slot")),
            };
            chromium(host, view)?;
            let (at, port) = point(message)?;
            let read = read_of(message)?;
            let sample = capture(host, view, &port, at, settings.loupe, &read)?;
            let mut answer = to_json(&sample, settings.format, false);
            let formats = Formats::of(sample.color);
            let mut picked = json!({
                "type": "picked",
                "view": view,
                "slot": slot,
                "color": formats.to_json(),
            });
            // The window image showed the display's colours: the panel says the value may not be
            // the page's own (RB-080-77).
            if matches!(read, Read::Window(_)) && message["exact"] == Value::Bool(false) {
                picked["managed"] = json!(true);
            }
            let value = formats.get(settings.format);
            answer["$post"] = json!([
                { "target": "panel", "message": picked },
                { "target": "all", "message": { "type": "stop" } },
                notice("picked", "success", &json!({ "value": value })),
            ]);
            Ok(answer)
        }
        Some("stopped") => Ok(stopped()),
        Some("chosen") => {
            let selector = message["selector"]
                .as_str()
                .ok_or_else(|| invalid("selector"))?;
            measure::measure(host, &json!({ "selector": selector, "target": "all" }))
        }
        _ => Err(invalid("message")),
    }
}

#[cfg(test)]
#[path = "page_tests.rs"]
pub(crate) mod tests;
