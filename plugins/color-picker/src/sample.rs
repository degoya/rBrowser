//! The pixel under a point: the MCP tool `color_sample` (`[[tools]] sample`) and the pipette's
//! loupe ([`crate::page`]).
//!
//! A point is in CSS pixels of the viewport, as MCP and the page's mouse events give it. The
//! component reads the view's image as its window shows it and keeps the pixels around the point
//! ([`crate::shot`], RB-080-64); the viewport's size, DPR and scroll offset, which the page
//! script sends and the tool reads with `evaluate` (`VIEWPORT`), map the point into the image -
//! or, where the window's image cannot be had, into a clip of the document.
//!
//! The pipette does not ask per position: it takes one [`tile`] where the pointer rests and
//! reads the loupe from it (RB-080-56), cut from the window's image by the [`Map`] the page script
//! measured once ([`crate::shot::calibrate_view`], RB-080-77) - or from a clip where the settings
//! ask for one (`color-picker.capture`).

use base64::Engine as _;
use rb_plugin_api::{Failure, FailureKind, PluginHost, ViewId};
use serde_json::{Value, json};

use crate::convert::{Format, Formats};
use crate::pixels::{
    MAX_TILE_BYTES, Sample, TILE_HALF, clip_around, sample, sample_mapped, tile_around,
};
use crate::settings::Settings;
pub use crate::shot::{Map, Marker, Viewport};
use crate::shot::{clipped_image, mapping, window_image};
use crate::targets::{self, Default};
use crate::{bad_data, failure, invalid};

/// The largest coordinate taken, in CSS px.
pub const MAX_COORDINATE: f64 = 100_000.0;

/// What `evaluate` answers about the viewport: its size, DPR and scroll offset.
pub const VIEWPORT: &str = "({ width: innerWidth, height: innerHeight, dpr: devicePixelRatio, \
    scrollX: scrollX, scrollY: scrollY })";

/// How the page script asks for a view to be read: from the window's image by the [`Map`] it
/// measured, or from a clip (the settings' `capture`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Read {
    Window(Map),
    Clip,
}

/// A coordinate: finite, 0 to [`MAX_COORDINATE`].
#[must_use]
pub fn coordinate(value: &Value) -> Option<f64> {
    value
        .as_f64()
        .filter(|number| number.is_finite() && (0.0..=MAX_COORDINATE).contains(number))
}

/// The sample of the viewport point `x`, `y` in `view`.
///
/// # Errors
///
/// The CDP failure; `invalid` for an answer that is no PNG; `outside` when the image does not
/// hold the point.
pub fn capture(
    host: &mut dyn PluginHost,
    view: ViewId,
    port: &Viewport,
    (x, y): (f64, f64),
    loupe: u32,
    read: &Read,
) -> Result<Sample, Failure> {
    if x >= port.width || y >= port.height {
        return Err(failure(FailureKind::NotFound, "outside"));
    }
    let taken = match read {
        Read::Window(map) => {
            let image = window_image(host, view, map, (x, y), loupe / 2)?;
            sample_mapped(&image, &mapping(map), (x, y), loupe)
        }
        Read::Clip => {
            let (x, y) = (x + port.scroll_x, y + port.scroll_y);
            let clip = clip_around(x, y, port.dpr, loupe);
            let (image, scale) = clipped_image(host, view, &clip, port.dpr)?;
            sample(&image, (clip.x, clip.y), scale, (x, y), loupe)
        }
    };
    taken.ok_or_else(|| failure(FailureKind::NotFound, "outside"))
}

/// The pipette's tile around the viewport point `x`, `y` in `view`: the pixels the page script
/// draws the loupe from for as long as the pointer stays inside, `rgb` its `width` x `height`
/// pixels as base64, three bytes each, at `scale` image pixels per CSS px. Cut from the window's
/// image: `left`, `top` its first pixel in that image of `imageWidth` x `imageHeight`, whose pixel
/// `offsetX`, `offsetY` shows the viewport's CSS point 0, 0. From a clip instead: `x`, `y` the
/// clip's origin in CSS px of the document.
///
/// # Errors
///
/// The CDP failure; `invalid` for an answer that is no PNG, a window image of another size than
/// the map's (`window_size`) or a tile larger than [`MAX_TILE_BYTES`].
pub fn tile(
    host: &mut dyn PluginHost,
    view: ViewId,
    port: &Viewport,
    (x, y): (f64, f64),
    read: &Read,
) -> Result<Value, Failure> {
    let (image, scale, mut answer) = match read {
        Read::Window(map) => {
            let image = window_image(host, view, map, (x, y), TILE_HALF)?;
            let region = image.region();
            let answer = json!({
                "left": region.left,
                "top": region.top,
                "offsetX": map.left,
                "offsetY": map.top,
                "imageWidth": image.width,
                "imageHeight": image.height,
            });
            (image, map.scale, answer)
        }
        Read::Clip => {
            let clip = tile_around(x + port.scroll_x, y + port.scroll_y, port.dpr);
            let (image, scale) = clipped_image(host, view, &clip, port.dpr)?;
            (image, scale, json!({ "x": clip.x, "y": clip.y }))
        }
    };
    let region = image.region();
    if image.rgb().len() > MAX_TILE_BYTES || region.width == 0 || region.height == 0 {
        return Err(bad_data("tile_size"));
    }
    answer["scale"] = json!(scale);
    answer["width"] = json!(region.width);
    answer["height"] = json!(region.height);
    answer["rgb"] = json!(base64::engine::general_purpose::STANDARD.encode(image.rgb()));
    Ok(answer)
}

/// A sample as JSON: the colour in every format, the device pixel it was read from
/// (`devicePixel`) and the image pixels per CSS px (`dpr`, the view's DPR when it is shown at it);
/// with `grid` also the loupe's cells (`#rrggbb` or `null`).
#[must_use]
pub fn to_json(sample: &Sample, format: Format, grid: bool) -> Value {
    let formats = Formats::of(sample.color);
    let mut answer = json!({
        "color": formats.to_json(),
        "value": formats.get(format),
        "dpr": (sample.scale * 1000.0).round() / 1000.0,
        "devicePixel": { "x": sample.device.0, "y": sample.device.1 },
    });
    if grid {
        answer["grid"] = sample
            .grid
            .iter()
            .map(|cell| cell.map_or(Value::Null, |color| Value::String(color.hex())))
            .collect();
    }
    answer
}

fn number(value: &Value) -> Option<f64> {
    value.as_f64().filter(|number| number.is_finite())
}

/// `color_sample`: the colour at the viewport point `x`, `y` of the view `target` (the active
/// view without one; `"all"` samples every view).
///
/// # Errors
///
/// `invalid` for bad arguments; the views' failure.
pub fn tool(host: &mut dyn PluginHost, args: &Value) -> Result<Value, Failure> {
    let x = coordinate(&args["x"]).ok_or_else(|| invalid("x"))?;
    let y = coordinate(&args["y"]).ok_or_else(|| invalid("y"))?;
    let settings = Settings::read(host);
    let views = targets::resolve(host, args, Default::Active)?;
    let mut answers = Vec::new();
    for view in views {
        if !targets::supported(&view) {
            answers.push(targets::unsupported(&view));
            continue;
        }
        let answer = viewport(host, view.id)
            .and_then(|port| capture(host, view.id, &port, (x, y), settings.loupe, &Read::Clip));
        answers.push(match answer {
            Ok(sample) => {
                let mut value = to_json(&sample, settings.format, false);
                value["view"] = json!(view.id);
                value["label"] = json!(view.label);
                value["x"] = json!(x);
                value["y"] = json!(y);
                value
            }
            Err(failure) => targets::failed(&view, &failure),
        });
    }
    Ok(json!({ "views": answers }))
}

/// The viewport of a view as `VIEWPORT` reads it.
fn viewport(host: &mut dyn PluginHost, view: ViewId) -> Result<Viewport, Failure> {
    let answer: Value =
        serde_json::from_str(&host.evaluate(view, VIEWPORT)?).map_err(|_| bad_data("viewport"))?;
    let read = |key: &str| number(&answer[key]).ok_or_else(|| bad_data("viewport"));
    let size = |key: &str| {
        read(key).and_then(|value| {
            (value > 0.0)
                .then_some(value)
                .ok_or_else(|| bad_data("viewport"))
        })
    };
    Ok(Viewport {
        width: size("width")?,
        height: size("height")?,
        dpr: size("dpr")?,
        // An overscrolled page reports a negative offset for a moment: the document's edge.
        scroll_x: read("scrollX")?.max(0.0),
        scroll_y: read("scrollY")?.max(0.0),
    })
}

#[cfg(test)]
#[path = "sample_tests.rs"]
mod tests;
