//! The focus order against the visible order (`focus-order-jump`): moving to the next focusable
//! element jumps back against the reading order (left to right, top to bottom) - up the page
//! without moving to a column to the right, or back to the left on the same line. That happens
//! where CSS reorders the page (`order`, `flex-direction: *-reverse`, grid placement) or a
//! positive `tabindex` pulls an element forward, and often only at some breakpoints. Elements
//! fixed or sticky on the screen, off the screen or without a box are left out.

use crate::page::{Focusable, PageOutline};

use super::{FOCUS_ORDER_JUMP, Page, Sink};

/// Whether `next` lies before `previous` in reading order: entirely above it and not in a column
/// to its right, or on the same line (overlapping by at least half the lower one's height) and
/// entirely to its left. Boxes are `x, y, width, height` in CSS px.
fn jumps_back(previous: [i64; 4], next: [i64; 4]) -> bool {
    let [px, py, pw, ph] = previous;
    let [nx, ny, nw, nh] = next;
    let above = ny + nh <= py;
    let right_column = nx >= px + pw;
    let overlap = (py + ph).min(ny + nh) - py.max(ny);
    let same_line = overlap > 0 && overlap * 2 >= ph.min(nh);
    let left = nx + nw <= px;
    (above && !right_column) || (same_line && left)
}

/// `link "Docs"`: the element's role (or tag) and name.
fn named(element: &Focusable) -> String {
    let kind = if element.role.is_empty() {
        &element.tag
    } else {
        &element.role
    };
    if element.name.is_empty() {
        kind.clone()
    } else {
        format!("{kind} \"{}\"", element.name)
    }
}

pub(super) fn check(sink: &mut Sink<'_>, page: Page<'_>, outline: &PageOutline) {
    let placed: Vec<&Focusable> = outline
        .focus
        .iter()
        .filter(|f| {
            f.at.visible && !f.at.hidden && !f.fixed && f.at.rect[2] > 0 && f.at.rect[3] > 0
        })
        .collect();
    for pair in placed.windows(2) {
        let [previous, next] = pair else {
            continue;
        };
        if jumps_back(previous.at.rect, next.at.rect) {
            sink.emit(
                FOCUS_ORDER_JUMP,
                page,
                Some(&next.at),
                &[("from", named(previous)), ("to", named(next))],
            );
        }
    }
}

#[cfg(test)]
#[path = "focus_tests.rs"]
mod tests;
