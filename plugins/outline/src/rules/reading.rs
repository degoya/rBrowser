//! The reading order against the source order (`reading-order`, WCAG 1.3.2, RB-094-02): the
//! headings and interactive elements in document order, and of each neighbouring pair the later
//! one that shows before the earlier one - left of it on the same line, or above it and not in a
//! column to its right; right to left where the content reads so. CSS `order`, grid placement,
//! `flex-direction: *-reverse` and absolute positioning do that; a screen reader and the
//! keyboard follow the source. The finding sits at the element out of place and names both;
//! `data` holds the other's selector and path, so the panel marks the pair.
//!
//! The focus order against the visible order is `focus-order-jump` (WCAG 2.4.3, [`super::focus`]).
//! Elements fixed or sticky on the screen, hidden, off the screen or without a box are left out.

use serde_json::json;

use crate::page::{Located, PageOutline};

use super::{Page, READING_ORDER, Sink};

/// One element of the reading sequence.
struct Placed<'a> {
    order: i64,
    rect: [i64; 4],
    label: String,
    at: &'a Located,
}

/// Whether `next` lies before `previous` in reading order: on the same line (overlapping by at
/// least half the lower one's height) and ending left of `previous`'s start, or entirely above it
/// and not in a column to its right. Boxes are `x, y, width, height` in CSS px, mirrored for right
/// to left; `tolerance` the px the two may overlap by and still count as one before the other.
#[must_use]
pub fn before(previous: [i64; 4], next: [i64; 4], tolerance: i64) -> bool {
    let [px, py, pw, ph] = previous;
    let [nx, ny, nw, nh] = next;
    let overlap = (py + ph).min(ny + nh) - py.max(ny);
    if overlap > 0 && overlap * 2 >= ph.min(nh) {
        return nx < px && nx + nw <= px + tolerance;
    }
    let above = ny < py && ny + nh <= py + tolerance;
    let right_column = nx >= px + pw;
    above && !right_column
}

/// The box as left to right reads it: mirrored for content that reads right to left.
fn reading_box(rect: [i64; 4], rtl: bool) -> [i64; 4] {
    let [x, y, width, height] = rect;
    if rtl {
        [-(x + width), y, width, height]
    } else {
        rect
    }
}

fn placed(at: &Located) -> bool {
    at.visible && !at.hidden && at.rect[2] > 0 && at.rect[3] > 0
}

/// `kind "name"`, or the kind alone.
fn named(kind: &str, name: &str) -> String {
    if name.is_empty() {
        kind.to_owned()
    } else {
        format!("{kind} \"{name}\"")
    }
}

/// The headings and interactive elements on the screen, in document order, each once.
fn sequence(outline: &PageOutline) -> Vec<Placed<'_>> {
    let rtl = outline.is_rtl();
    let headings = outline
        .headings
        .iter()
        .filter(|h| placed(&h.at))
        .map(|h| Placed {
            order: h.order,
            rect: reading_box(h.at.rect, rtl),
            label: named("heading", if h.text.is_empty() { &h.name } else { &h.text }),
            at: &h.at,
        });
    let focus = outline
        .focus
        .iter()
        .filter(|f| placed(&f.at) && !f.fixed)
        .map(|f| Placed {
            order: f.order,
            rect: reading_box(f.at.rect, rtl),
            label: named(if f.role.is_empty() { &f.tag } else { &f.role }, &f.name),
            at: &f.at,
        });
    let mut all: Vec<Placed<'_>> = headings.chain(focus).collect();
    all.sort_by_key(|one| one.order);
    // A focusable heading is in both lists.
    all.dedup_by(|later, earlier| later.order == earlier.order && later.at.path == earlier.at.path);
    all
}

pub(super) fn check(sink: &mut Sink<'_>, page: Page<'_>, outline: &PageOutline, tolerance: i64) {
    if !sink.wants(READING_ORDER) {
        return;
    }
    let sequence = sequence(outline);
    for pair in sequence.windows(2) {
        let [previous, next] = pair else {
            continue;
        };
        if before(previous.rect, next.rect, tolerance) {
            let other =
                json!({ "other": { "selector": previous.at.selector, "path": previous.at.path } });
            sink.emit_with(
                READING_ORDER,
                page,
                Some(next.at),
                &[("from", previous.label.clone()), ("to", next.label.clone())],
                Some(other.to_string()),
            );
        }
    }
}

#[cfg(test)]
#[path = "reading_tests.rs"]
mod tests;
