//! The screenshots the colour picker takes of a view (RB-080-64, RB-080-77).
//!
//! **The window's image, no clip.** Chromium renders a clipped `Page.captureScreenshot` through
//! the device emulation, and so - when rBrowser's emulation scales the view into its cell or sets
//! another DPR - an unclipped one from the surface: the view shows that picture for a frame, a
//! flicker at every screenshot (on Windows: the view dark but for a lit square of the clip's
//! size, owner's screens of 2026-11-15). The window's image (`fromSurface: false`, no clip) is
//! what the view shows and is taken without rendering anew. Measured in Chromium with the
//! emulation on the reading session, as rBrowser's views have it (DPR 3, scale 0.5; five shots
//! each, frames other than the page): with a clip 5, unclipped from the surface 5, from the
//! window 0 (RB-080-64's job file has the probe and its numbers).
//!
//! **Where the viewport lies in it** is measured once per pipette and view ([`calibrate_view`],
//! [`crate::calibrate`]): an image that is the viewport at its DPR maps it from its size
//! (RB-090-19) unless the markers in it deny it - then, as any other, by the markers the page
//! script shows in the viewport's corners and beside the pointer (RB-090-31, RB-090-40: a view
//! the window's edge cuts); the [`Map`] it answers comes back with every later read
//! ([`window_image`]), which keeps only the region around the point. Why a view could not be
//! mapped is a word of [`Unmapped`] the page script hands to the status bar. A window image of
//! another size than the map's (the cell changed) is refused, and the page script measures again. [`clipped_image`], RB-080-56's way, is read only where a person
//! chose it (`color-picker.capture`, exact page colours on a display with a colour profile) and
//! for MCP's `color_sample`, which has no page script to show the markers. Every screenshot is
//! logged - their number is what a flickering view is traced by; a measurement at info, so a
//! person's log names how each view is read.

use rb_plugin_api::{Failure, PluginHost, ViewId};
use serde_json::{Value, json};

use crate::bad_data;
use crate::calibrate::{Band, Finder, calibrate, direct};
pub use crate::calibrate::{MARKER, Map, Marker};
use crate::color::Rgba;
use crate::pixels::{
    Clip, Image, MAX_PIXELS, Mapping, Region, Rgb, decode, decode_region, decode_scan, image_size,
};
use crate::visible::{Area, visible};

/// A view's viewport: its size in CSS px, its DPR and its scroll offset.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Viewport {
    pub width: f64,
    pub height: f64,
    pub dpr: f64,
    pub scroll_x: f64,
    pub scroll_y: f64,
}

/// Image pixels per CSS pixel of an image `width` pixels wide of `css` px: the DPR when the image
/// is them at it, rounded (Chromium's 116 px for 77 CSS px at 1.5), so the mapping does not drift
/// by a pixel across the image; otherwise the image's width over the CSS width.
#[must_use]
pub fn image_scale(width: u32, css: f64, dpr: f64) -> f64 {
    let width = f64::from(width);
    if (width - css * dpr).abs() <= 1.0 {
        dpr
    } else {
        width / css
    }
}

/// The window image's parameters: what the view shows, never rendered anew.
fn window_params() -> Value {
    json!({ "format": "png", "captureBeyondViewport": false, "fromSurface": false })
}

fn hex(rgb: Rgb) -> String {
    Rgba {
        r: f64::from(rgb[0]),
        g: f64::from(rgb[1]),
        b: f64::from(rgb[2]),
        a: 1.0,
    }
    .hex()
}

/// Why a window image does not map the viewport, the word the page script hands back with
/// `unreadable` for the status bar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unmapped {
    /// No image: the CDP call failed, or its answer is no PNG.
    Image,
    /// More than [`MAX_PIXELS`]: neither searched for markers nor read.
    Size,
    /// The view is shown scaled and its markers do not map it.
    Markers,
}

impl Unmapped {
    /// The word of the page script's `unreadable`.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Self::Image => "image",
            Self::Size => "size",
            Self::Markers => "markers",
        }
    }
}

/// A failure for the log: its code and its reason.
fn described(failure: &Failure) -> String {
    let code = failure.code.as_deref().unwrap_or("-");
    match failure.params.get("reason") {
        Some(reason) => format!("{code} ({reason})"),
        None => code.to_owned(),
    }
}

/// The boxes for the log: at most six, `left,top wxh`.
fn listed(bands: &[Band]) -> String {
    let mut shown: Vec<String> = bands
        .iter()
        .take(6)
        .map(|band| {
            format!(
                "{},{} {}x{}",
                band.left,
                band.top,
                band.right - band.left,
                band.bottom - band.top
            )
        })
        .collect();
    if bands.len() > 6 {
        shown.push("...".to_owned());
    }
    format!("[{}]", shown.join("; "))
}

/// Where the viewport `port` lies in the window image of `view`, the page script showing its
/// markers at `marker`; whether the image's colours are the page's (on a display with a colour
/// profile they are not). The whole image is searched for the markers: an image that is the
/// viewport at its DPR is mapped by its size unless the boxes deny it (diag12: the page drawn
/// smaller in its corner; RB-090-40: the view cut by the window's edge, its bottom markers higher
/// than its size puts them, its top ones gone). Without a box at all its size maps it only where
/// the page script drew two markers beside the person's pointer (`pointed`), which lies where the
/// view is seen: then the markers are not seen on this page, not cut off. Logged at info either
/// way. With the map the part of the viewport the image is known to show ([`visible`], where the
/// loupe goes).
///
/// # Errors
///
/// Why the image does not map the viewport ([`Unmapped`]).
pub fn calibrate_view(
    host: &mut dyn PluginHost,
    view: ViewId,
    port: &Viewport,
    marker: &[(f64, f64)],
    pointed: bool,
) -> Result<(Map, bool, Area), Unmapped> {
    let failed = |host: &mut dyn PluginHost, failure: &Failure| {
        host.log(
            "info",
            &format!(
                "color-picker: the window image of view {view} failed: {}",
                described(failure)
            ),
        );
        Unmapped::Image
    };
    let data =
        screenshot(host, view, &window_params()).map_err(|failure| failed(host, &failure))?;
    let image = image_size(&data).map_err(|failure| failed(host, &failure))?;
    let (width, height) = (port.width, port.height);
    let size = format!(
        "{} x {} px for {width} x {height} CSS px at DPR {}",
        image.0, image.1, port.dpr
    );
    // RB-090-19, diag11: a view 2000 CSS px wide at DPR 1.5 and more than 1864 high.
    if u64::from(image.0) * u64::from(image.1) > MAX_PIXELS {
        host.log(
            "info",
            &format!(
                "color-picker: the window image of view {view} ({size}) has more than the {} megapixels the pipette reads",
                MAX_PIXELS >> 20
            ),
        );
        return Err(Unmapped::Size);
    }
    let dims = (width, height, port.dpr);
    let mut finder = Finder::default();
    let mut visit = |y: u32, row: &[Rgb]| finder.row(y, row);
    let decoded = decode_scan(
        &data,
        |_, _| Region::around(-1, -1, 0, 1, 1),
        Some(&mut visit),
    );
    if let Err(failure) = decoded {
        return Err(failed(host, &failure));
    }
    let bands = finder.markers();
    // RB-090-40: no box and no pointer in the view - the markers may all be cut off, and an image
    // moved to the window's edge is not where its size puts it.
    if bands.is_empty() && !pointed && direct(dims, image).is_some() {
        host.log(
            "info",
            &format!(
                "color-picker: the window image of view {view} ({size}) shows none of the markers and the pointer is not in the view: measured again where it points"
            ),
        );
        return Err(Unmapped::Markers);
    }
    match calibrate(&bands, marker, dims, image) {
        Ok((map, exact)) => {
            let how = if direct(dims, image) == Some(map) {
                "the image's size".to_owned()
            } else {
                format!("its markers {}", listed(&bands))
            };
            let colours = match bands.first() {
                _ if exact => "the page's colours".to_owned(),
                Some(band) => format!(
                    "the display's colours (#ff00ff, #00ff00 read {}, {})",
                    hex(band.colours[0]),
                    hex(band.colours[1])
                ),
                None => "no marker seen: the colours may be the display's".to_owned(),
            };
            host.log(
                "info",
                &format!(
                    "color-picker: view {view} is read from its window image ({size}): the viewport at {:.4} px per CSS px from ({}, {}) by {how}, {colours}",
                    map.scale, map.left, map.top
                ),
            );
            Ok((map, exact, visible(&map, &bands, marker, (width, height))))
        }
        Err(why) => {
            let drawn: Vec<String> = marker.iter().map(|(x, y)| format!("{x},{y}")).collect();
            host.log(
                "info",
                &format!(
                    "color-picker: the window image of view {view} ({size}) {why}: boxes {}, markers drawn at [{}] CSS px",
                    listed(&bands),
                    drawn.join("; ")
                ),
            );
            Err(Unmapped::Markers)
        }
    }
}

/// The window's image of `view` around the viewport point `x`, `y` by `map`: `half` image pixels
/// on each side. Logged at debug, so loops do not fill the log.
///
/// # Errors
///
/// The CDP failure; `invalid` for an answer that is no PNG, `window_size` for an image of another
/// size than `map`'s (measure again).
pub fn window_image(
    host: &mut dyn PluginHost,
    view: ViewId,
    map: &Map,
    at: (f64, f64),
    half: u32,
) -> Result<Image, Failure> {
    let (px, py) = mapping(map).point(at);
    let shot = screenshot(host, view, &window_params()).and_then(|data| {
        decode_region(&data, |width, height| {
            if (width, height) == (map.width, map.height) {
                Region::around(px, py, half, width, height)
            } else {
                Region::around(-1, -1, 0, 1, 1)
            }
        })
    });
    let message = match &shot {
        Ok(image) if (image.width, image.height) != (map.width, map.height) => format!(
            "the window image of view {view} is {} x {} px, not the {} x {} px measured; measured again",
            image.width, image.height, map.width, map.height
        ),
        Ok(image) => format!(
            "view {view} read from its window image, {} x {} px",
            image.width, image.height
        ),
        Err(failure) => format!(
            "the window image of view {view} failed: {}",
            failure.code.as_deref().unwrap_or("-")
        ),
    };
    host.log("debug", &format!("color-picker: {message}"));
    let image = shot?;
    if (image.width, image.height) != (map.width, map.height) {
        return Err(bad_data("window_size"));
    }
    Ok(image)
}

/// How CSS px of the viewport map into the window image by `map`.
#[must_use]
pub fn mapping(map: &Map) -> Mapping {
    Mapping {
        origin: (0.0, 0.0),
        offset: (map.left, map.top),
        scale: map.scale,
    }
}

/// The screenshot of `clip` (CSS px of the document) in `view` and its image pixels per CSS px.
/// The view flickers for a frame; logged at debug.
///
/// # Errors
///
/// The CDP failure; `invalid` for an answer that is no PNG.
pub fn clipped_image(
    host: &mut dyn PluginHost,
    view: ViewId,
    clip: &Clip,
    dpr: f64,
) -> Result<(Image, f64), Failure> {
    let params = json!({
        "format": "png",
        "clip": { "x": clip.x, "y": clip.y, "width": clip.width, "height": clip.height, "scale": 1 },
        "captureBeyondViewport": false,
        "fromSurface": true,
    });
    host.log(
        "debug",
        &format!(
            "color-picker: screenshot of view {view}, clip {} x {} CSS px",
            clip.width, clip.height
        ),
    );
    let image = screenshot(host, view, &params).and_then(|data| decode(&data));
    if let Err(failure) = &image {
        let code = failure.code.as_deref().unwrap_or("-");
        host.log(
            "debug",
            &format!("color-picker: screenshot of view {view} failed: {code}"),
        );
    }
    let image = image?;
    let scale = image_scale(image.width, clip.width, dpr);
    Ok((image, scale))
}

/// The base64 PNG of one `Page.captureScreenshot` with `params`.
fn screenshot(host: &mut dyn PluginHost, view: ViewId, params: &Value) -> Result<String, Failure> {
    let answer = host.cdp_send(view, "Page.captureScreenshot", &params.to_string())?;
    let mut answer: Value = serde_json::from_str(&answer).map_err(|_| bad_data("screenshot"))?;
    match answer.get_mut("data").map(Value::take) {
        Some(Value::String(data)) => Ok(data),
        _ => Err(bad_data("screenshot")),
    }
}

#[cfg(test)]
#[path = "shot_tests.rs"]
mod tests;
