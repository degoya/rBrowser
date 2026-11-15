//! Draws the annotations onto the image, in their order, and crops last. The geometry (arrow
//! heads, markers, the highlight's opacity) is the panel's too (`web/src/panel/draw.ts`), so the
//! preview and the file agree.

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

fn polyline(pixmap: &mut Pixmap, points: &[[f32; 2]], color: Rgb, width: f32) {
    let mut builder = PathBuilder::new();
    let Some(first) = points.first() else {
        return;
    };
    builder.move_to(first[0], first[1]);
    if points.len() == 1 {
        if let Some(dot) = PathBuilder::from_circle(first[0], first[1], width / 2.0) {
            pixmap.fill_path(
                &dot,
                &paint(color, 255),
                FillRule::Winding,
                Transform::identity(),
                None,
            );
        }
        return;
    }
    for point in &points[1..] {
        builder.line_to(point[0], point[1]);
    }
    if let Some(path) = builder.finish() {
        pixmap.stroke_path(
            &path,
            &paint(color, 255),
            &stroke(width, true),
            Transform::identity(),
            None,
        );
    }
}

/// A line from `from` to the base of a filled head whose tip is `to`.
fn arrow(pixmap: &mut Pixmap, from: [f32; 2], to: [f32; 2], color: Rgb, width: f32) {
    let (dx, dy) = (to[0] - from[0], to[1] - from[1]);
    let length = dx.hypot(dy);
    if length < 1.0 {
        return;
    }
    let (ux, uy) = (dx / length, dy / length);
    let head = (width * ARROW_HEAD_WIDTHS).max(ARROW_HEAD_MIN).min(length);
    let base = [to[0] - ux * head, to[1] - uy * head];
    polyline(pixmap, &[from, base], color, width);
    let half = head / 2.0;
    let mut builder = PathBuilder::new();
    builder.move_to(to[0], to[1]);
    builder.line_to(base[0] - uy * half, base[1] + ux * half);
    builder.line_to(base[0] + uy * half, base[1] - ux * half);
    builder.close();
    if let Some(path) = builder.finish() {
        pixmap.fill_path(
            &path,
            &paint(color, 255),
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }
}

fn fill_rect(pixmap: &mut Pixmap, rect: [f32; 4], color: Rgb, alpha: u8) {
    if let Some(rect) = Rect::from_xywh(rect[0], rect[1], rect[2], rect[3]) {
        pixmap.fill_rect(rect, &paint(color, alpha), Transform::identity(), None);
    }
}

fn marker(pixmap: &mut Pixmap, x: f32, y: f32, number: u32, color: Rgb, size: f32) {
    if let Some(circle) = PathBuilder::from_circle(x, y, size * MARKER_RADIUS) {
        pixmap.fill_path(
            &circle,
            &paint(color, 255),
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }
    let label = number.to_string();
    let em = size * MARKER_TEXT;
    let left = x - text::width(&label, em) / 2.0;
    let top = y + DIGIT_HALF * em - em * text::ASCENT / text::UNITS_PER_EM;
    text::draw(pixmap, left, top, &label, em, Rgb(255, 255, 255));
}

/// `annotations` drawn onto `pixmap` in order, the crop (if any) applied last. A crop that
/// leaves nothing of the image is `bad_annotation` with reason `crop`.
pub fn render(mut pixmap: Pixmap, annotations: &[Annotation]) -> Result<Pixmap, Failure> {
    let (width, height) = (pixmap.width(), pixmap.height());
    let mut crop = None;
    for (index, annotation) in annotations.iter().enumerate() {
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
                    fill_rect(&mut pixmap, [*x, *y, *w, *h], *color, 255);
                } else if let Some(rect) = Rect::from_xywh(*x, *y, *w, *h) {
                    let path = PathBuilder::from_rect(rect);
                    pixmap.stroke_path(
                        &path,
                        &paint(*color, 255),
                        &stroke(*width, false),
                        Transform::identity(),
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
            } => arrow(&mut pixmap, [*x1, *y1], [*x2, *y2], *color, *width),
            Annotation::Line {
                x1,
                y1,
                x2,
                y2,
                color,
                width,
            } => polyline(&mut pixmap, &[[*x1, *y1], [*x2, *y2]], *color, *width),
            Annotation::Freehand {
                points,
                color,
                width,
            } => polyline(&mut pixmap, points, *color, *width),
            Annotation::Text {
                x,
                y,
                text: words,
                color,
                size,
            } => text::draw(&mut pixmap, *x, *y, words, *size, *color),
            Annotation::Marker {
                x,
                y,
                number,
                color,
                size,
            } => marker(&mut pixmap, *x, *y, *number, *color, *size),
            Annotation::Highlight { x, y, w, h, color } => {
                fill_rect(&mut pixmap, [*x, *y, *w, *h], *color, HIGHLIGHT_ALPHA);
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
                        RedactMode::Pixelate => redact::pixelate(&mut pixmap, region, *block),
                        RedactMode::Blur => redact::blur(&mut pixmap, region, *block),
                        RedactMode::Black => redact::black(&mut pixmap, region),
                    }
                }
            }
            Annotation::Crop { x, y, w, h } => {
                let region = Region::clip(*x, *y, *w, *h, width, height)
                    .ok_or_else(|| bad_annotation(index, "crop"))?;
                crop = Some((index, region));
            }
        }
    }
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
