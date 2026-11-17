//! The active view's label strip in the accent colour (`ui.highlightActiveView`, RB-080-04,
//! `crate::layout::active_label`): grid, focus, horizontal and split draw native label strips,
//! and the leader's is drawn in the shell's solid primary with the theme's text for it and a
//! sign before its label (`crate::app_theme::ACTIVE_SIGN`, RB-090-22) - told by more than its
//! colour, 3 : 1 off its neighbours, its text AA in both themes (`crate::app_theme::palette`).
//! The canvas board marks its own labels. A strip the grid builds anew, or whose text changes,
//! gets the mark from [`style`]. UI thread only.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;

use cef::*;
use rb_engine::ViewId;

use crate::app_theme::{marked, opaque};

thread_local! {
    /// The active view, as the host said last.
    static ACTIVE: Cell<Option<ViewId>> = const { Cell::new(None) };
    /// `ui.highlightActiveView`.
    static ENABLED: Cell<bool> = const { Cell::new(true) };
    /// Each strip's own background, to put back when its view stops leading.
    static NEUTRAL: RefCell<HashMap<ViewId, u32>> = RefCell::new(HashMap::new());
}

/// `view` leads now: the old leader's strip loses the mark, the new one's gets it.
pub fn mark(view: Option<ViewId>) {
    ACTIVE.with(|active| active.set(view));
    restyle();
}

/// Switches the mark on or off.
pub fn set_enabled(enabled: bool) {
    ENABLED.with(|current| current.set(enabled));
    restyle();
}

/// Styles the strip `label` of `view` (the grid just built it, its text changed, or the leader
/// changed): ground, text colour and the sign before its label.
pub fn style(view: ViewId, label: &LabelButton) {
    let neutral = NEUTRAL.with(|neutral| {
        *neutral
            .borrow_mut()
            .entry(view)
            .or_insert_with(|| label.background_color())
    });
    let leads = ENABLED.with(Cell::get) && ACTIVE.with(Cell::get) == Some(view);
    let colors = super::theme::colors();
    if leads {
        label.set_background_color(opaque(colors.mark_ground));
        label.set_enabled_text_colors(opaque(colors.mark_text));
    } else {
        label.set_background_color(neutral);
        label.set_enabled_text_colors(opaque(colors.text));
    }
    let text = CefString::from(&label.text()).to_string();
    let shown = marked(&text, leads);
    if shown != text {
        label.set_text(Some(&CefString::from(shown.as_str())));
    }
}

/// The theme of the strip `label` of `view` changed (RB-080-47): it came into a window, or
/// the window's theme changed. CEF put the theme's ground over whatever [`style`] had set -
/// a theme change resets a view's background (`cef_view_t::set_background_color`) - so the
/// ground the strip has now is its neutral one, and the leader's gets its mark again. From the
/// strip's delegate (`label_strip.rs`), as the footer's does it.
pub fn theme_changed(view: ViewId, label: &LabelButton) {
    NEUTRAL.with(|neutral| neutral.borrow_mut().insert(view, label.background_color()));
    style(view, label);
}

fn restyle() {
    super::cell_labels::each(style);
}
