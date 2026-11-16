//! `popOut` of the user settings (RB-080-14, Layout > Pop-out): how a view popped out of the
//! grid into a window of its own behaves. Read by `crates/rbrowser/src/pop_out`.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// `popOut`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct PopOutSettings {
    /// The grid keeps the popped-out view's cell with a placeholder (a click brings the window
    /// to the front); off, the other views close the gap.
    pub keep_placeholder: bool,
    /// A window larger than the screen is cut to it and the page shown at a zoom below 1, named
    /// in the title; off, the window keeps the real size beyond the screen.
    pub fit_to_screen: bool,
    /// Each breakpoint's window opens where and as large as it was last closed.
    pub remember_bounds: bool,
}

impl Default for PopOutSettings {
    fn default() -> Self {
        Self {
            keep_placeholder: true,
            fit_to_screen: true,
            remember_bounds: true,
        }
    }
}

#[cfg(test)]
#[path = "pop_out_settings_tests.rs"]
mod tests;
