//! The layout modes horizontal, split and canvas (RB-080-04).
//!
//! Each mode is a pure function "preset + grid area + scroll/pan/zoom -> where each view goes"
//! ([`horizontal::arrange`], [`split::arrange`], [`canvas::arrange`]), tested without CEF. The
//! window places the views at these rectangles as overlay views (`cef_app/placed.rs`): the
//! rectangles come from here, exactly, and a view that would not lie wholly inside the grid area
//! is not shown (and is throttled like any view without area, RB-020-09) - in the canvas only one
//! wholly outside, a view past the edge is cut to the area by the window (RB-080-55) - so a view
//! never covers the toolbar strip or the DevTools dock. Grid and focus stay the nested BoxLayouts of
//! `layout_plan` (ADR 0001, Decision 2).
//!
//! A mode switch keeps the views: the host matches them by breakpoint as for a preset, and only
//! their bounds and scale change (`host::placed`). Which breakpoints have a view at all is the
//! hidden selection `layout.hidden`, the same in every mode (`view_edit`, RB-080-41).

pub mod actions;
pub mod active_label;
pub mod canvas;
pub mod canvas_board;
pub mod canvas_hold;
pub mod canvas_level;
pub mod canvas_live;
pub mod canvas_ops;
pub mod canvas_zoom;
pub mod defs;
pub mod horizontal;
pub mod ipc;
pub mod middle_pan;
pub mod native_menu;
pub mod native_menu_entries;
pub mod save_preset;
pub mod spec;
pub mod split;
pub mod store;
#[cfg(test)]
pub(crate) mod test_support;
mod view_add_size;
pub mod view_edit;
pub mod view_menu;
mod view_menu_open;
mod view_selection;
pub mod zoom_menu;

use rb_engine::CellRect;

pub use spec::{ModeSpec, resolve_mode};

/// A view to place: its breakpoint and the breakpoint's viewport in CSS px.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    /// The breakpoint id.
    pub breakpoint: String,
    /// Viewport width in CSS px.
    pub width: u32,
    /// Viewport height in CSS px.
    pub height: u32,
}

/// Where one view goes, in DIP of the window's client area: the label strip above it, the view,
/// and whether it is shown (wholly inside the grid area; in the canvas partly, RB-080-55).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Slot {
    /// The label strip.
    pub label: CellRect,
    /// The view.
    pub rect: CellRect,
    /// Shown; a hidden view keeps its browser and is throttled.
    pub visible: bool,
}

impl Slot {
    /// The strip and the view together: what the window places - a label band right above the
    /// view, also when the canvas holds a cut view's label at the area's top edge (RB-080-55).
    pub fn outer(&self) -> CellRect {
        CellRect {
            x: self.label.x,
            y: self.rect.y.saturating_sub_unsigned(self.label.height),
            width: self.rect.width.max(self.label.width),
            height: self.label.height + self.rect.height,
        }
    }
}

/// `inner` lies wholly inside `outer`.
pub fn contains(outer: CellRect, inner: CellRect) -> bool {
    let right = |rect: CellRect| i64::from(rect.x) + i64::from(rect.width);
    let bottom = |rect: CellRect| i64::from(rect.y) + i64::from(rect.height);
    inner.x >= outer.x
        && inner.y >= outer.y
        && right(inner) <= right(outer)
        && bottom(inner) <= bottom(outer)
}

/// Two rectangles share area.
pub fn overlaps(a: CellRect, b: CellRect) -> bool {
    let (a_right, b_right) = (
        i64::from(a.x) + i64::from(a.width),
        i64::from(b.x) + i64::from(b.width),
    );
    let (a_bottom, b_bottom) = (
        i64::from(a.y) + i64::from(a.height),
        i64::from(b.y) + i64::from(b.height),
    );
    i64::from(a.x) < b_right
        && i64::from(b.x) < a_right
        && i64::from(a.y) < b_bottom
        && i64::from(b.y) < a_bottom
}

/// A strip of `label` DIP at the top of `outer` and the view below it.
pub(crate) fn split_label(outer: CellRect, label: u32, visible: bool) -> Slot {
    let strip = label.min(outer.height);
    Slot {
        label: CellRect {
            height: strip,
            ..outer
        },
        rect: CellRect {
            y: outer.y + strip as i32,
            height: outer.height - strip,
            ..outer
        },
        visible,
    }
}

/// `value` rounded to a whole DIP and held to `u32`.
pub(crate) fn to_dip(value: f64) -> u32 {
    if value.is_finite() && value > 0.0 {
        // In range: checked against u32::MAX first.
        value.round().min(f64::from(u32::MAX)) as u32
    } else {
        0
    }
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "sequence_tests.rs"]
mod sequence_tests;

#[cfg(test)]
#[path = "canvas_resize_tests.rs"]
mod canvas_resize_tests;

#[cfg(test)]
#[path = "canvas_drag_tests.rs"]
mod canvas_drag_tests;
