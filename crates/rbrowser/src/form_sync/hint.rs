//! The skip hint (RB-020-05): while `Leader S N` is armed, the active view's label strip - the
//! grid gap above the view - says that its next click is not synced; it goes when the click
//! came, Escape took the skip back, or another view became active.

use std::sync::Mutex;

use rb_engine::{ViewHost, ViewId};

use crate::cdp::lock;
use crate::host::Host;

/// The hint's text; `{label}` is the view's label.
pub const SKIP_HINT_KEY: &str = "sync.skipNext.hint";

/// The label strip's text of a view labelled `label` while its next click is skipped.
pub fn hint_label(label: &str) -> String {
    rb_i18n::t_with(SKIP_HINT_KEY, &[("label", label)])
}

/// Which view's strip shows the hint.
#[derive(Debug, Default)]
pub struct SkipHint {
    shown: Mutex<Option<ViewId>>,
}

impl SkipHint {
    /// The hint on `armed` (the active view while the skip is armed; `None`: nowhere). A view
    /// that showed it gets its label back.
    pub fn show(&self, host: &Host, armed: Option<ViewId>) {
        let mut shown = lock(&self.shown);
        if *shown == armed {
            return;
        }
        if let Some(info) = shown.and_then(|view| host.view(view)) {
            host.ui().set_cell_label(info.id, info.label, None);
        }
        if let Some(info) = armed.and_then(|view| host.view(view)) {
            host.ui()
                .set_cell_label(info.id, hint_label(&info.label), None);
        }
        *shown = armed;
    }
}

#[cfg(test)]
#[path = "hint_tests.rs"]
mod tests;
