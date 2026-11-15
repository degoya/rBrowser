//! Text and marker numbers, drawn with the font the panel shows too (`font/annotation-font.ttf`,
//! Noto Sans Mono Bold, a subset under the SIL Open Font License, `panel/annotation-font.OFL.txt`). A
//! text's `size` is its em in pixels, as CSS's `font-size`; its box's top is the ascender line,
//! so the baseline is `y + size * ASCENT / UNITS_PER_EM` - `web/src/panel/draw.ts` uses the same
//! numbers.

use ab_glyph::{Font, FontRef, PxScale, point};
use tiny_skia::Pixmap;

use crate::model::Rgb;

/// The font, embedded.
pub const FONT: &[u8] = include_bytes!("../font/annotation-font.ttf");
/// The font's units per em, ascender and descender (its `hhea` table).
pub const UNITS_PER_EM: f32 = 1000.0;
pub const ASCENT: f32 = 1069.0;
pub const DESCENT: f32 = -293.0;
/// Every glyph's advance in em (a monospaced font).
pub const ADVANCE: f32 = 0.6;

fn font() -> Option<FontRef<'static>> {
    FontRef::try_from_slice(FONT).ok()
}

/// The width of `text` at `size` in pixels.
#[must_use]
pub fn width(text: &str, size: f32) -> f32 {
    text.chars().count() as f32 * ADVANCE * size
}

/// The height of one line of text at `size`: ascender to descender.
#[must_use]
pub fn line_height(size: f32) -> f32 {
    size * (ASCENT - DESCENT) / UNITS_PER_EM
}

/// Blends `coverage` (0-1) of the opaque `color` into the pixel at `x`, `y`.
fn blend(pixmap: &mut Pixmap, x: i64, y: i64, coverage: f32, color: Rgb) {
    let (width, height) = (i64::from(pixmap.width()), i64::from(pixmap.height()));
    if x < 0 || y < 0 || x >= width || y >= height || coverage <= 0.0 {
        return;
    }
    let alpha = coverage.min(1.0);
    let i = ((y * width + x) * 4) as usize;
    let data = pixmap.data_mut();
    for (channel, source) in [color.0, color.1, color.2, u8::MAX].into_iter().enumerate() {
        // Premultiplied: the source's alpha is one.
        let mixed = f32::from(source) * alpha + f32::from(data[i + channel]) * (1.0 - alpha);
        data[i + channel] = mixed.round().clamp(0.0, 255.0) as u8;
    }
}

/// Draws `text` with its box's top left at `x`, `y`, `size` pixels to the em, in `color`. A
/// character the font lacks is drawn as its missing-glyph box.
pub fn draw(pixmap: &mut Pixmap, x: f32, y: f32, text: &str, size: f32, color: Rgb) {
    let Some(font) = font() else {
        return;
    };
    let scale = PxScale::from(size * (ASCENT - DESCENT) / UNITS_PER_EM);
    let baseline = y + size * ASCENT / UNITS_PER_EM;
    let mut pen = x;
    for c in text.chars() {
        let glyph = font
            .glyph_id(c)
            .with_scale_and_position(scale, point(pen, baseline));
        if let Some(outlined) = font.outline_glyph(glyph) {
            let bounds = outlined.px_bounds();
            outlined.draw(|gx, gy, coverage| {
                let px = bounds.min.x as i64 + i64::from(gx);
                let py = bounds.min.y as i64 + i64::from(gy);
                blend(pixmap, px, py, coverage, color);
            });
        }
        pen += ADVANCE * size;
    }
}

/// Whether the embedded font reads and its metrics are the constants above.
#[must_use]
pub fn metrics_match() -> bool {
    font().is_some_and(|font| {
        font.units_per_em() == Some(UNITS_PER_EM)
            && font.ascent_unscaled() == ASCENT
            && font.descent_unscaled() == DESCENT
            && font.h_advance_unscaled(font.glyph_id('W')) == ADVANCE * UNITS_PER_EM
    })
}

#[cfg(test)]
#[path = "text_tests.rs"]
mod tests;
