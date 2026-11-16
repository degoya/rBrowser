//! `ui.highlightActiveView` (RB-080-04): the window marks the active view's label strip in the
//! accent colour - grid, focus, horizontal and split; the canvas board marks its own labels. The
//! host marks each new active view (`Host::activate`); this module hands the window the switch
//! and the view active at the start.

use rb_engine::ViewHost as _;

use crate::control::Core;

/// The core is up: the window gets the switch and the active view.
pub fn on_start(core: &Core) {
    on_user_settings_changed(core);
    core.ui().mark_active(core.host().active_view());
}

/// The user settings changed: the window follows `ui.highlightActiveView`.
pub fn on_user_settings_changed(core: &Core) {
    core.ui()
        .set_active_highlight(core.user_settings().ui.highlight_active_view);
}

#[cfg(test)]
#[path = "active_label_tests.rs"]
mod tests;
