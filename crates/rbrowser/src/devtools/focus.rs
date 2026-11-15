//! Which keyboard focus a key-down comes from. The CEF keyboard handlers name their browser's
//! source; DevTools' own browsers (the dock, the DevTools window) report
//! [`Focus::Devtools`], so `shortcuts.disableInDevtools` lets their keys - `Ctrl+Shift+C` and
//! every other DevTools shortcut - through untouched (handed over from wave 0.1-w1a, where no
//! key ever came from DevTools).

use rb_commands::Focus;

/// The browser a key-down arrived in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeySource {
    /// The shell view (the toolbar).
    Shell,
    /// The canvas board behind the canvas's views (RB-080-94).
    Board,
    /// A view of the grid.
    View,
    /// The docked frontend or the DevTools window.
    Devtools,
}

impl KeySource {
    /// The focus the leader state machine hears of.
    pub fn focus(self) -> Focus {
        match self {
            KeySource::Shell => Focus::Shell,
            KeySource::Board => Focus::Board,
            KeySource::View => Focus::View,
            KeySource::Devtools => Focus::Devtools,
        }
    }
}

#[cfg(test)]
#[path = "focus_tests.rs"]
mod tests;
