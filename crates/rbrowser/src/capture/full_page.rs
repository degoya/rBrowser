//! The plan of a full-page capture (RB-099-11): what one shot of a Chromium view can hold and
//! how a longer page is taken in sections.
//!
//! A shot is copied out of one GPU texture of at most [`MAX_DEVICE_PX`] device pixels a side;
//! what lies beyond it repeats the top of the image. The limit is in device pixels, so the
//! ratio of device to CSS pixels counts: a breakpoint without `dpr` renders at the screen's, which
//! [`device_ratio`] measures from `Page.getLayoutMetrics` (`contentSize` is in device pixels,
//! `cssContentSize` in CSS px) rather than assuming 1.
//!
//! [`plan`] cuts the page into sections of at most [`SECTION_DEVICE_PX`] rows and
//! [`MAX_SHOT_PIXELS`] pixels each. Every section starts and ends on a whole device row (its CSS
//! height a multiple of the smallest step, up to 32 CSS px, whose rows are whole at the ratio), so
//! the stitched image has no gap and no doubled row; at a ratio without such a step (a browser
//! zoom on top) a seam is within one row, which the stitcher evens out. The image is bounded too: at most [`MAX_IMAGE_ROWS`] rows
//! and [`MAX_IMAGE_PIXELS`] pixels (what `rb_visual` decodes, so visual regression can compare
//! it); a longer page is cut there and the plan says so (`truncated`).
//!
//! One emulated viewport serves every section: the page's own height when it fits one section
//! (as before RB-099-11), else one section's height - never a viewport taller than a texture.
//! The sections are clips of that one layout at their offsets (`captureBeyondViewport`), so a
//! fixed or sticky element is painted where that layout puts it - once, not again in every
//! section.

use serde_json::Value;

/// The longest side of one shot in device pixels (a GPU texture).
pub const MAX_DEVICE_PX: f64 = 16_384.0;
/// The rows of one section: below [`MAX_DEVICE_PX`] by the rounding of a measured ratio (at most
/// [`SNAP_TOLERANCE`]) and of the last row.
pub const SECTION_DEVICE_PX: f64 = 16_000.0;
/// The most pixels of one shot at its pixel ratio: 64 megapixels (3840 x 16384 at 1).
pub const MAX_SHOT_PIXELS: f64 = 64.0 * 1024.0 * 1024.0;
/// The most rows of a stitched image (`rb_visual::raster::MAX_SIDE`).
pub const MAX_IMAGE_ROWS: f64 = rb_visual::raster::MAX_SIDE as f64;
/// The most pixels of a stitched image (`rb_visual::raster::MAX_PIXELS`, 256 megapixels).
pub const MAX_IMAGE_PIXELS: f64 = rb_visual::raster::MAX_PIXELS as f64;
/// The most sections of one capture.
pub const MAX_SECTIONS: usize = 16;
/// How far a measured ratio may lie from the breakpoint's or a multiple of 1/8 to count as it.
const SNAP_TOLERANCE: f64 = 0.005;
/// The measured ratios taken at all; outside, the measurement is not believed.
const RATIO_RANGE: std::ops::RangeInclusive<f64> = 0.1..=16.0;
/// The largest CSS step searched for whole rows.
const MAX_STEP: u32 = 32;

/// The device pixels per CSS pixel a view renders at: measured from `metrics`
/// (`Page.getLayoutMetrics`) - the breakpoint's `dpr` when it lies within the rounding of whole
/// device pixels, else the measurement, on a multiple of 1/8 when that close - else `dpr`, else 1.
pub fn device_ratio(metrics: &Value, dpr: f64) -> f64 {
    let configured = positive(dpr);
    match (measured(metrics), configured) {
        (Some(ratio), Some(dpr)) if (ratio - dpr).abs() <= dpr * SNAP_TOLERANCE * 2.0 => dpr,
        (Some(ratio), _) => {
            let eighths = (ratio * 8.0).round() / 8.0;
            if (ratio - eighths).abs() <= ratio * SNAP_TOLERANCE {
                eighths
            } else {
                ratio
            }
        }
        (None, Some(dpr)) => dpr,
        (None, None) => 1.0,
    }
}

/// Device over CSS px of the widest measure `metrics` gives (the most precise one).
fn measured(metrics: &Value) -> Option<f64> {
    const PAIRS: [(&str, &str); 3] = [
        ("/contentSize/height", "/cssContentSize/height"),
        ("/contentSize/width", "/cssContentSize/width"),
        (
            "/layoutViewport/clientWidth",
            "/cssLayoutViewport/clientWidth",
        ),
    ];
    let mut best: Option<(f64, f64)> = None;
    for (device, css) in PAIRS {
        let (Some(device), Some(css)) = (
            metrics.pointer(device).and_then(Value::as_f64),
            metrics.pointer(css).and_then(Value::as_f64),
        ) else {
            continue;
        };
        if !(device.is_finite() && css.is_finite() && device > 0.0 && css >= 1.0) {
            continue;
        }
        if best.is_none_or(|(_, widest)| css > widest) {
            best = Some((device / css, css));
        }
    }
    best.map(|(ratio, _)| ratio)
        .filter(|ratio| RATIO_RANGE.contains(ratio))
}

/// `value` when it is a finite number above 0.
fn positive(value: f64) -> Option<f64> {
    (value.is_finite() && value > 0.0).then_some(value)
}

/// One shot of a full page: a clip of the page in CSS px.
#[derive(Clone, Debug, PartialEq)]
pub struct Section {
    /// The top in CSS px of the page.
    pub y: f64,
    /// The height in CSS px.
    pub height: f64,
    /// The image rows it gives: its rounded end less its rounded start (may be 0 below a
    /// ratio of 1).
    pub rows: u32,
}

/// How a full page is captured.
#[derive(Clone, Debug, PartialEq)]
pub struct FullPagePlan {
    /// The emulated viewport's height in CSS px for every section.
    pub viewport: f64,
    /// The height captured in CSS px, from the top.
    pub height: f64,
    /// The page's height in CSS px.
    pub page: f64,
    /// The shots, top to bottom, back to back.
    pub sections: Vec<Section>,
    /// The page is taller than the image may be: it ends at `height`.
    pub truncated: bool,
}

impl FullPagePlan {
    /// The rows of the whole image.
    pub fn rows(&self) -> u32 {
        self.sections.iter().map(|section| section.rows).sum()
    }
}

/// The plan of a page `content` CSS px tall (at least `viewport`) in a view `width` CSS px wide
/// at `ratio` device px per CSS px and an output `scale`. A ratio below 1 (or none) counts as 1
/// against every limit, so a checked-in `dpr` never raises them. `None` when not even one CSS
/// row fits the limits (a view absurdly wide at its ratio).
pub fn plan(
    content: f64,
    viewport: f64,
    width: f64,
    ratio: f64,
    scale: f64,
) -> Option<FullPagePlan> {
    let width = positive(width)?;
    let ratio = positive(ratio).unwrap_or(1.0);
    let scale = positive(scale).unwrap_or(1.0);
    // Image rows per CSS px; per CSS px against the image's limits; and against a texture's,
    // which never counts on a shot scaled down after it was painted (the overview's CSS px).
    let out = ratio * scale;
    let image = ratio.max(1.0) * scale;
    let texture = ratio.max(1.0) * scale.max(1.0);
    let step = whole_step(out);
    let texture_row = (width * texture).ceil().max(1.0);
    let section_rows = SECTION_DEVICE_PX.min((MAX_SHOT_PIXELS / texture_row).floor());
    let section = whole_multiple((section_rows / texture).floor(), step)?;
    let image_row = (width * image).ceil().max(1.0);
    let image_rows = MAX_IMAGE_ROWS.min((MAX_IMAGE_PIXELS / image_row).floor());
    let most =
        whole_multiple((image_rows / image).floor(), step)?.min(section * MAX_SECTIONS as f64);
    let viewport = positive(viewport).unwrap_or(1.0);
    let page = if content.is_finite() {
        content.max(viewport)
    } else {
        viewport
    }
    .ceil();
    let (height, truncated) = if page <= most {
        ((page / step).ceil() * step, false)
    } else {
        (most, true)
    };
    // Rows counted from the top (the rounded end less the rounded start), so the sections sum to
    // the whole image's rounded rows: at a ratio without a whole-row step, sections rounded on
    // their own could add up past `MAX_IMAGE_ROWS` (code review, wave 099-w36). A section stays
    // within `section_rows`, as rounding is monotonic and whole rows shift it by whole rows.
    let mut sections = Vec::new();
    let mut y = 0.0;
    while y < height {
        let part = section.min(height - y);
        let rows = ((y + part) * out).round() - (y * out).round();
        sections.push(Section {
            y,
            height: part,
            rows: rows.max(0.0) as u32,
        });
        y += part;
    }
    Some(FullPagePlan {
        viewport: height.min(section),
        height,
        page,
        sections,
        truncated,
    })
}

/// The smallest CSS height (up to [`MAX_STEP`]) whose image rows at `out` are whole; else 1.
fn whole_step(out: f64) -> f64 {
    (1..=MAX_STEP)
        .map(f64::from)
        .find(|step| {
            let rows = step * out;
            (rows - rows.round()).abs() < 1e-6
        })
        .unwrap_or(1.0)
}

/// `value` down to a multiple of `step`; `None` below one step.
fn whole_multiple(value: f64, step: f64) -> Option<f64> {
    let whole = (value / step).floor() * step;
    (whole >= step).then_some(whole)
}

#[cfg(test)]
#[path = "full_page_tests.rs"]
mod tests;
