//! Draws the annotations onto the image, in their order, and crops last. The geometry (arrow
//! heads, markers, the highlight's opacity) is the panel's too (`web/src/panel/draw.ts`), so the
//! preview and the file agree.
//!
//! [`draw_all`] draws onto a band of the image as well (RB-099-24, `crate::band`): its row 0 is
//! the image's row `top`, every shape moved up by `top` whole pixels. Shapes and rectangles are
//! filled as paths either way (a rectangle too: tiny-skia's own rectangle fill rounds otherwise
//! than its path fill), so a band draws what the whole image draws.

use rb_plugin_api::Failure;
use tiny_skia::{
    Color, FillRule, IntRect, LineCap, LineJoin, Paint, PathBuilder, Pixmap, Rect, Stroke,
    Transform,
};

use crate::model::{Annotation, RedactMode, Rgb, bad_annotation};
use crate::redact::{self, Region};
use crate::text;

/// The highlight's opacity, out of 255.
pub const HIGHLIGHT_ALPHA: u8 = 89;
/// An arrow head's length: this many stroke widths, at least [`ARROW_HEAD_MIN`] pixels.
pub const ARROW_HEAD_WIDTHS: f32 = 4.0;
pub const ARROW_HEAD_MIN: f32 = 12.0;
/// A marker's radius and its number's size, in parts of its `size`.
pub const MARKER_RADIUS: f32 = 0.75;
pub const MARKER_TEXT: f32 = 0.8;
/// Half a digit's height in em: the number's baseline lies this far below the centre.
pub const DIGIT_HALF: f32 = 0.357;

fn paint(color: Rgb, alpha: u8) -> Paint<'static> {
    let mut paint = Paint::default();
    paint.set_color(Color::from_rgba8(color.0, color.1, color.2, alpha));
    paint.anti_alias = true;
    paint
}

fn stroke(width: f32, round: bool) -> Stroke {
    Stroke {
        width,
        line_cap: if round { LineCap::Round } else { LineCap::Butt },
        line_join: if round {
            LineJoin::Round
        } else {
            LineJoin::Miter
        },
        ..Stroke::default()
    }
}

fn polyline(pixmap: &mut Pixmap, at: Transform, points: &[[f32; 2]], color: Rgb, width: f32) {
    let mut builder = PathBuilder::new();
    let Some(first) = points.first() else {
        return;
    };
    builder.move_to(first[0], first[1]);
    if points.len() == 1 {
        if let Some(dot) = PathBuilder::from_circle(first[0], first[1], width / 2.0) {
            pixmap.fill_path(&dot, &paint(color, 255), FillRule::Winding, at, None);
        }
        return;
    }
    for point in &points[1..] {
        builder.line_to(point[0], point[1]);
    }
    if let Some(path) = builder.finish() {
        pixmap.stroke_path(&path, &paint(color, 255), &stroke(width, true), at, None);
    }
}

/// A line from `from` to the base of a filled head whose tip is `to`.
fn arrow(pixmap: &mut Pixmap, at: Transform, from: [f32; 2], to: [f32; 2], color: Rgb, width: f32) {
    let (dx, dy) = (to[0] - from[0], to[1] - from[1]);
    let length = dx.hypot(dy);
    if length < 1.0 {
        return;
    }
    let (ux, uy) = (dx / length, dy / length);
    let head = (width * ARROW_HEAD_WIDTHS).max(ARROW_HEAD_MIN).min(length);
    let base = [to[0] - ux * head, to[1] - uy * head];
    polyline(pixmap, at, &[from, base], color, width);
    let half = head / 2.0;
    let mut builder = PathBuilder::new();
    builder.move_to(to[0], to[1]);
    builder.line_to(base[0] - uy * half, base[1] + ux * half);
    builder.line_to(base[0] + uy * half, base[1] - ux * half);
    builder.close();
    if let Some(path) = builder.finish() {
        pixmap.fill_path(&path, &paint(color, 255), FillRule::Winding, at, None);
    }
}

fn fill_rect(pixmap: &mut Pixmap, at: Transform, rect: [f32; 4], color: Rgb, alpha: u8) {
    if let Some(rect) = Rect::from_xywh(rect[0], rect[1], rect[2], rect[3]) {
        let path = PathBuilder::from_rect(rect);
        pixmap.fill_path(&path, &paint(color, alpha), FillRule::Winding, at, None);
    }
}

/// `at` moves a shape up to the pixmap's rows; the number's glyphs move as far (`-at.ty`).
fn marker(pixmap: &mut Pixmap, at: Transform, x: f32, y: f32, number: u32, color: Rgb, size: f32) {
    if let Some(circle) = PathBuilder::from_circle(x, y, size * MARKER_RADIUS) {
        pixmap.fill_path(&circle, &paint(color, 255), FillRule::Winding, at, None);
    }
    let label = number.to_string();
    let em = size * MARKER_TEXT;
    let left = x - text::width(&label, em) / 2.0;
    let glyph_top = y + DIGIT_HALF * em - em * text::ASCENT / text::UNITS_PER_EM + at.ty;
    text::draw(pixmap, left, glyph_top, &label, em, Rgb(255, 255, 255));
}

/// The crop of `annotations` (the last one, as the panel applies it) as the whole pixels it
/// keeps of a `width` x `height` image, with its index; a crop that leaves nothing of the image
/// is `bad_annotation` with reason `crop`.
pub fn crop_of(
    annotations: &[Annotation],
    width: u32,
    height: u32,
) -> Result<Option<(usize, Region)>, Failure> {
    let mut crop = None;
    for (index, annotation) in annotations.iter().enumerate() {
        if let Annotation::Crop { x, y, w, h } = annotation {
            let region = Region::clip(*x, *y, *w, *h, width, height)
                .ok_or_else(|| bad_annotation(index, "crop"))?;
            crop = Some((index, region));
        }
    }
    Ok(crop)
}

/// Every annotation but the crop drawn in order onto `pixmap`, which holds the rows from `top`
/// of an image `height` rows high (and as wide as the pixmap).
pub fn draw_all(pixmap: &mut Pixmap, top: u32, height: u32, annotations: &[Annotation]) {
    let width = pixmap.width();
    let at = Transform::from_translate(0.0, -(top as f32));
    let shift = top as f32;
    for annotation in annotations {
        match annotation {
            Annotation::Rect {
                x,
                y,
                w,
                h,
                color,
                width,
                fill,
            } => {
                if *fill {
                    fill_rect(pixmap, at, [*x, *y, *w, *h], *color, 255);
                } else if let Some(rect) = Rect::from_xywh(*x, *y, *w, *h) {
                    let path = PathBuilder::from_rect(rect);
                    pixmap.stroke_path(
                        &path,
                        &paint(*color, 255),
                        &stroke(*width, false),
                        at,
                        None,
                    );
                }
            }
            Annotation::Arrow {
                x1,
                y1,
                x2,
                y2,
                color,
                width,
            } => arrow(pixmap, at, [*x1, *y1], [*x2, *y2], *color, *width),
            Annotation::Line {
                x1,
                y1,
                x2,
                y2,
                color,
                width,
            } => polyline(pixmap, at, &[[*x1, *y1], [*x2, *y2]], *color, *width),
            Annotation::Freehand {
                points,
                color,
                width,
            } => polyline(pixmap, at, points, *color, *width),
            Annotation::Text {
                x,
                y,
                text: words,
                color,
                size,
            } => text::draw(pixmap, *x, *y - shift, words, *size, *color),
            Annotation::Marker {
                x,
                y,
                number,
                color,
                size,
            } => marker(pixmap, at, *x, *y, *number, *color, *size),
            Annotation::Highlight { x, y, w, h, color } => {
                fill_rect(pixmap, at, [*x, *y, *w, *h], *color, HIGHLIGHT_ALPHA);
            }
            Annotation::Redact {
                x,
                y,
                w,
                h,
                mode,
                block,
            } => {
                if let Some(region) = Region::clip(*x, *y, *w, *h, width, height) {
                    match mode {
                        RedactMode::Pixelate => redact::pixelate(pixmap, region, *block, top),
                        RedactMode::Blur => redact::blur(pixmap, region, *block, top),
                        RedactMode::Black => redact::black(pixmap, region, top),
                    }
                }
            }
            Annotation::Crop { .. } => {}
        }
    }
}

/// `annotations` drawn onto `pixmap` in order, the crop (if any) applied last. A crop that
/// leaves nothing of the image is `bad_annotation` with reason `crop`.
pub fn render(mut pixmap: Pixmap, annotations: &[Annotation]) -> Result<Pixmap, Failure> {
    let (width, height) = (pixmap.width(), pixmap.height());
    let crop = crop_of(annotations, width, height)?;
    draw_all(&mut pixmap, 0, height, annotations);
    let Some((index, region)) = crop else {
        return Ok(pixmap);
    };
    IntRect::from_ltrb(
        region.left as i32,
        region.top as i32,
        region.right as i32,
        region.bottom as i32,
    )
    .and_then(|rect| pixmap.clone_rect(rect))
    .ok_or_else(|| bad_annotation(index, "crop"))
}

#[cfg(test)]
#[path = "render_tests.rs"]
mod tests;
