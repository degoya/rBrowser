//! The annotation document: what the panel draws and the component renders, the same JSON in
//! both (`<name>.annotations.json`, MCP `shot_annotate`'s `annotations`). Coordinates are image
//! pixels with the origin top left; a rectangle may be dragged in any direction and is held with a
//! positive size after [`validate`].

use rb_plugin_api::{Failure, FailureKind};
use serde::{Deserialize, Serialize};

use crate::PLUGIN_ID;

/// The version of the annotations file.
pub const DOCUMENT_VERSION: u32 = 1;
/// The most annotations one image carries.
pub const MAX_ANNOTATIONS: usize = 1000;
/// The most points of one freehand stroke.
pub const MAX_POINTS: usize = 5000;
/// The most points of every freehand stroke together.
pub const MAX_TOTAL_POINTS: usize = 20_000;
/// The longest text, in characters.
pub const MAX_TEXT_CHARS: usize = 200;
/// The largest coordinate or size, either sign.
pub const MAX_COORDINATE: f32 = 100_000.0;
/// A stroke's width in pixels.
pub const STROKE_WIDTH: (f32, f32) = (1.0, 64.0);
/// A text's or marker's size in pixels.
pub const TEXT_SIZE: (f32, f32) = (6.0, 256.0);
/// A mosaic block's side in pixels; a smaller block is raised to the minimum (blocks of 4 px leave
/// text readable).
pub const BLOCK: (u32, u32) = (8, 128);
/// The block when the annotation names none.
pub const DEFAULT_BLOCK: u32 = 12;
/// The highest marker number.
pub const MAX_MARKER: u32 = 9999;

/// An opaque colour, `#rrggbb` in JSON.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Rgb(pub u8, pub u8, pub u8);

impl TryFrom<String> for Rgb {
    type Error = String;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        Rgb::parse(&text).ok_or_else(|| "colour".to_owned())
    }
}

impl From<Rgb> for String {
    fn from(color: Rgb) -> Self {
        format!("#{:02x}{:02x}{:02x}", color.0, color.1, color.2)
    }
}

impl Rgb {
    /// `#rrggbb` (either case); anything else is none.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let hex = text.strip_prefix('#')?;
        if hex.len() != 6 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return None;
        }
        let channel = |at: usize| u8::from_str_radix(hex.get(at..at + 2)?, 16).ok();
        Some(Self(channel(0)?, channel(2)?, channel(4)?))
    }
}

/// How a redaction hides what is under it; every mode is rendered into the pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RedactMode {
    /// Each block becomes its average colour.
    #[default]
    Pixelate,
    /// The mosaic, then smoothed: it looks blurred but holds only the blocks' averages.
    Blur,
    /// Solid black.
    Black,
}

/// One annotation; `type` names the tool.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase", deny_unknown_fields)]
pub enum Annotation {
    /// A rectangle's outline, or the rectangle filled (`fill`).
    Rect {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        color: Rgb,
        width: f32,
        #[serde(default)]
        fill: bool,
    },
    /// A line with a head at its end.
    Arrow {
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        color: Rgb,
        width: f32,
    },
    Line {
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        color: Rgb,
        width: f32,
    },
    Freehand {
        points: Vec<[f32; 2]>,
        color: Rgb,
        width: f32,
    },
    /// One line of text, its box's top left at `x`, `y`.
    Text {
        x: f32,
        y: f32,
        text: String,
        color: Rgb,
        size: f32,
    },
    /// A filled circle with a number, centred at `x`, `y`.
    Marker {
        x: f32,
        y: f32,
        number: u32,
        color: Rgb,
        size: f32,
    },
    /// A translucent fill.
    Highlight {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        color: Rgb,
    },
    Redact {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        #[serde(default)]
        mode: RedactMode,
        #[serde(default = "default_block")]
        block: u32,
    },
    /// The part of the image the result keeps; at most one, applied last.
    Crop { x: f32, y: f32, w: f32, h: f32 },
}

const fn default_block() -> u32 {
    DEFAULT_BLOCK
}

/// `<name>.annotations.json`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Document {
    pub version: u32,
    /// The original, project-relative (`.rbrowser/out/screenshots/.../mobile.png`).
    pub image: String,
    pub width: u32,
    pub height: u32,
    pub annotations: Vec<Annotation>,
}

/// `plugin.screenshot-editor.errors.bad_annotation`: annotation `index` breaks `reason`.
#[must_use]
pub fn bad_annotation(index: usize, reason: &str) -> Failure {
    Failure::coded(
        FailureKind::BadData,
        &format!("plugin.{PLUGIN_ID}.errors.bad_annotation"),
    )
    .with_param("index", index)
    .with_param("reason", reason)
}

fn coordinate(values: &[f32]) -> bool {
    values
        .iter()
        .all(|value| value.is_finite() && value.abs() <= MAX_COORDINATE)
}

fn within(value: f32, (low, high): (f32, f32)) -> bool {
    value.is_finite() && value >= low && value <= high
}

/// The rectangle `x`, `y`, `w`, `h` with a positive size: one dragged up or left starts at its
/// other corner.
fn normalise(x: &mut f32, y: &mut f32, w: &mut f32, h: &mut f32) {
    if *w < 0.0 {
        *x += *w;
        *w = -*w;
    }
    if *h < 0.0 {
        *y += *h;
        *h = -*h;
    }
}

/// Checks every annotation against the limits above and normalises the rectangles; the first
/// that does not fit is the failure, with its index and what is wrong.
pub fn validate(annotations: &mut [Annotation]) -> Result<(), Failure> {
    if annotations.len() > MAX_ANNOTATIONS {
        return Err(bad_annotation(MAX_ANNOTATIONS, "count"));
    }
    let mut points_total = 0usize;
    let mut crops = 0usize;
    for (index, annotation) in annotations.iter_mut().enumerate() {
        let problem = match annotation {
            Annotation::Rect {
                x, y, w, h, width, ..
            } => {
                normalise(x, y, w, h);
                check_rect(*x, *y, *w, *h).or_else(|| stroke(*width))
            }
            Annotation::Highlight { x, y, w, h, .. } => {
                normalise(x, y, w, h);
                check_rect(*x, *y, *w, *h)
            }
            Annotation::Redact {
                x, y, w, h, block, ..
            } => {
                normalise(x, y, w, h);
                *block = (*block).max(BLOCK.0);
                check_rect(*x, *y, *w, *h).or_else(|| (*block > BLOCK.1).then_some("block"))
            }
            Annotation::Crop { x, y, w, h } => {
                normalise(x, y, w, h);
                crops += 1;
                check_rect(*x, *y, *w, *h).or_else(|| (crops > 1).then_some("crop"))
            }
            Annotation::Arrow {
                x1,
                y1,
                x2,
                y2,
                width,
                ..
            }
            | Annotation::Line {
                x1,
                y1,
                x2,
                y2,
                width,
                ..
            } => (!coordinate(&[*x1, *y1, *x2, *y2]))
                .then_some("coordinate")
                .or_else(|| stroke(*width)),
            Annotation::Freehand { points, width, .. } => {
                points_total += points.len();
                if points.is_empty() || points.len() > MAX_POINTS || points_total > MAX_TOTAL_POINTS
                {
                    Some("points")
                } else if !points.iter().all(|point| coordinate(point)) {
                    Some("coordinate")
                } else {
                    stroke(*width)
                }
            }
            Annotation::Text {
                x, y, text, size, ..
            } => check_text(*x, *y, text, *size),
            Annotation::Marker {
                x, y, number, size, ..
            } => (!coordinate(&[*x, *y]))
                .then_some("coordinate")
                .or_else(|| (*number > MAX_MARKER).then_some("number"))
                .or_else(|| (!within(*size, TEXT_SIZE)).then_some("size")),
        };
        if let Some(reason) = problem {
            return Err(bad_annotation(index, reason));
        }
    }
    Ok(())
}

fn check_rect(x: f32, y: f32, w: f32, h: f32) -> Option<&'static str> {
    (!coordinate(&[x, y, w, h])).then_some("coordinate")
}

fn stroke(width: f32) -> Option<&'static str> {
    (!within(width, STROKE_WIDTH)).then_some("width")
}

fn check_text(x: f32, y: f32, text: &str, size: f32) -> Option<&'static str> {
    if !coordinate(&[x, y]) {
        Some("coordinate")
    } else if text.trim().is_empty()
        || text.chars().count() > MAX_TEXT_CHARS
        || text.chars().any(char::is_control)
    {
        Some("text")
    } else if !within(size, TEXT_SIZE) {
        Some("size")
    } else {
        None
    }
}

#[cfg(test)]
#[path = "model_tests.rs"]
mod tests;
