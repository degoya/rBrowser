//! The overview image: every view side by side in layout order, each under a strip with its
//! label, its size (`393 x 852 @3x`), engine and environment, framed in the environment's
//! colour. The views out of sight are captured like the others (the app lifts their throttling
//! for the capture); this module only draws.

use crate::VisualError;
use crate::font::{GLYPH_HEIGHT, draw_text};
use crate::raster::Rgba;

/// The space around the whole image, in pixels.
pub const MARGIN: u32 = 24;
/// The space between two views.
pub const GAP: u32 = 24;
/// The height of the label strip above each view.
pub const STRIP: u32 = 52;
/// The environment frame's width around a view.
pub const FRAME: u32 = 3;
/// Pixels per font pixel.
const TEXT_SCALE: u32 = 2;
/// The background.
pub const BACKGROUND: [u8; 4] = [243, 244, 246, 255];
/// The frame of a view without an environment colour.
pub const BORDER: [u8; 4] = [209, 213, 219, 255];
/// The label.
const INK: [u8; 4] = [17, 24, 39, 255];
/// The size and detail line.
const INK_SOFT: [u8; 4] = [75, 85, 99, 255];

/// One view of the overview.
#[derive(Clone, Debug, PartialEq)]
pub struct Tile {
    /// The view's label (breakpoint label or id).
    pub label: String,
    /// `393 x 852 @3x` ([`crate::size_text`]).
    pub size: String,
    /// Engine and environment (`chromium \u{b7} staging`).
    pub detail: String,
    /// The environment's colour; a grey line when `None`.
    pub color: Option<[u8; 3]>,
    /// The capture, at CSS pixel size.
    pub image: Rgba,
}

/// How the overview is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OverviewOptions {
    /// The label strips (`capture.overviewLabels`).
    pub labels: bool,
}

/// Where the views go.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Placement {
    /// The image's width.
    pub width: u32,
    /// The image's height.
    pub height: u32,
    /// The top left of each view's capture, in order.
    pub positions: Vec<(u32, u32)>,
}

/// The placement of views of `sizes` (width, height): one row, top-aligned, in order.
pub fn layout(sizes: &[(u32, u32)], labels: bool) -> Placement {
    let top = MARGIN + if labels { STRIP } else { 0 };
    let mut x = MARGIN;
    let mut positions = Vec::with_capacity(sizes.len());
    for (index, (width, _)) in sizes.iter().enumerate() {
        if index > 0 {
            x = x.saturating_add(GAP);
        }
        positions.push((x, top));
        x = x.saturating_add(*width);
    }
    let tallest = sizes.iter().map(|(_, height)| *height).max().unwrap_or(0);
    Placement {
        width: x.saturating_add(MARGIN),
        height: top.saturating_add(tallest).saturating_add(MARGIN),
        positions,
    }
}

/// Draws the overview of `tiles`.
pub fn compose(tiles: &[Tile], options: &OverviewOptions) -> Result<Rgba, VisualError> {
    if tiles.is_empty() {
        return Err(VisualError::Empty);
    }
    let sizes: Vec<(u32, u32)> = tiles
        .iter()
        .map(|tile| (tile.image.width(), tile.image.height()))
        .collect();
    let placed = layout(&sizes, options.labels);
    let mut canvas = Rgba::new(placed.width, placed.height, BACKGROUND)?;
    let line = GLYPH_HEIGHT * TEXT_SCALE + 8;
    for (tile, (x, y)) in tiles.iter().zip(&placed.positions) {
        let (width, height) = (tile.image.width(), tile.image.height());
        let frame = match tile.color {
            Some([r, g, b]) => (FRAME, [r, g, b, 255]),
            None => (1, BORDER),
        };
        canvas.fill(
            x - frame.0,
            y - frame.0,
            width + 2 * frame.0,
            height + 2 * frame.0,
            frame.1,
        );
        canvas.blit(&tile.image, *x, *y);
        if options.labels {
            let text_top = y - STRIP + 6;
            let room = width.max(GAP);
            draw_text(
                &mut canvas,
                (*x, text_top),
                &tile.label,
                TEXT_SCALE,
                INK,
                room,
            );
            let second = if tile.detail.is_empty() {
                tile.size.clone()
            } else {
                format!("{} \u{b7} {}", tile.size, tile.detail)
            };
            draw_text(
                &mut canvas,
                (*x, text_top + line),
                &second,
                TEXT_SCALE,
                INK_SOFT,
                room,
            );
        }
    }
    Ok(canvas)
}

/// `#rrggbb` as RGB; `None` for anything else.
pub fn parse_color(text: &str) -> Option<[u8; 3]> {
    let hex = text.strip_prefix('#')?;
    if hex.len() != 6 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let channel = |at: usize| u8::from_str_radix(&hex[at..at + 2], 16).ok();
    Some([channel(0)?, channel(2)?, channel(4)?])
}

#[cfg(test)]
#[path = "overview_tests.rs"]
mod tests;
