//! `ui.statusBar` of the user settings (RB-080-15): the status bar at the bottom of the main
//! window - shown or not, how long an info or success message stays, how many messages the
//! history keeps, the canvas zoom at its right (RB-080-94). Settings > Layout > Status bar.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The fewest messages the history keeps.
pub const MIN_HISTORY_SIZE: u32 = 10;
/// The most messages the history keeps.
pub const MAX_HISTORY_SIZE: u32 = 1000;
/// The longest an info or success message stays, in seconds.
pub const MAX_INFO_TIMEOUT_SEC: u32 = 600;

/// `ui.statusBar`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct StatusBarSettings {
    /// Show the status bar at the bottom of the window; hidden, the grid takes its room and the
    /// messages still reach the history.
    pub visible: bool,
    /// How long an info or success message stays in the bar, in seconds; 0 keeps it until the
    /// next one. Warnings and errors stay until closed.
    #[schemars(range(max = 600))]
    pub info_timeout_sec: u32,
    /// How many messages the history keeps, oldest dropped first.
    #[schemars(range(min = 10, max = 1000))]
    pub history_size: u32,
    /// Show the canvas zoom at the right of the status bar while the canvas layout is shown: the
    /// active view's page zoom, a slider, the minus and plus buttons and the presets (RB-080-94).
    pub canvas_zoom: bool,
}

impl Default for StatusBarSettings {
    fn default() -> Self {
        Self {
            visible: true,
            info_timeout_sec: 6,
            history_size: 200,
            canvas_zoom: true,
        }
    }
}

impl StatusBarSettings {
    /// The history's size held to [`MIN_HISTORY_SIZE`]..=[`MAX_HISTORY_SIZE`] (a hand-edited file
    /// may say anything).
    #[must_use]
    pub fn history_capacity(&self) -> usize {
        let size = self.history_size.clamp(MIN_HISTORY_SIZE, MAX_HISTORY_SIZE);
        usize::try_from(size).unwrap_or(usize::MAX)
    }
}

#[cfg(test)]
#[path = "status_bar_settings_tests.rs"]
mod tests;
