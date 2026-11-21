//! `performance` of the user settings (RB-020-09): the throttling of views out of sight and of
//! visible views that are not active (Layout > Grid), the memory warning (Advanced >
//! Experimental flags) and the timing counters (Advanced > Diagnostics, RB-090-32). Read by
//! `crates/rbrowser/src/perf`.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The highest `performance.inactiveFps`; 60 and above is no limit.
pub const MAX_INACTIVE_FPS: u32 = 60;
/// The highest `performance.memoryWarningMb` (64 GiB).
pub const MAX_MEMORY_WARNING_MB: u32 = 65_536;

/// `performance`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct PerformanceSettings {
    /// A view without area on screen is reported hidden to Chromium, which stops painting it
    /// and throttles its timers; navigation and scroll sync still reach it, and it catches up
    /// with the leader's URL and scroll position when it shows again. Off, such views keep
    /// rendering.
    pub throttle_hidden: bool,
    /// Visible views that are not the active one run with their main thread slowed to about
    /// this many frames per second of work (CDP CPU throttling, 60 / value); 0 (or 60) leaves
    /// them at full speed. Sync still reaches them, a little later.
    #[schemars(range(max = 60))]
    pub inactive_fps: u32,
    /// The shell warns once when rBrowser's processes together use more private memory than
    /// this many MiB; 0 turns the warning off.
    #[schemars(range(max = 65_536))]
    pub memory_warning_mb: u32,
    /// The timing counters of the sync hub, the plugin calls and the shell IPC record (RB-090-32):
    /// the MCP tool `app_perf` and the diagnostics ZIP (`perf.json`) read them. Off, nothing is
    /// recorded; what was recorded stays readable.
    pub counters: bool,
}

impl Default for PerformanceSettings {
    fn default() -> Self {
        Self {
            throttle_hidden: true,
            inactive_fps: 0,
            // The K3 budget: eight views below 3 GB.
            memory_warning_mb: 3072,
            counters: true,
        }
    }
}

impl PerformanceSettings {
    /// The settings with every number in its range (a hand-edited file is not validated).
    pub fn clamped(self) -> Self {
        Self {
            inactive_fps: self.inactive_fps.min(MAX_INACTIVE_FPS),
            memory_warning_mb: self.memory_warning_mb.min(MAX_MEMORY_WARNING_MB),
            ..self
        }
    }
}

#[cfg(test)]
#[path = "perf_settings_tests.rs"]
mod tests;
