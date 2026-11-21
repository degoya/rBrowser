//! The page clock and the animation rates in the user settings (RB-094-07, Settings >
//! Emulation > Clock and animations): what the clock shortcut sets in every view - a start time
//! and a speed - and the animation rates the animations shortcut steps through, with the step
//! that moves paused animations on. The clock itself and the rates are live state of the window
//! (`rbrowser::time_control`), never written to a file.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The rates `overlays.animations` steps through by default: stopped, 10 %, 25 %, 100 %.
pub const DEFAULT_ANIMATION_RATES: [f64; 4] = [0.0, 0.1, 0.25, 1.0];
/// Every rate the settings offer, ascending.
pub const ANIMATION_RATE_CHOICES: [f64; 7] = [0.0, 0.05, 0.1, 0.25, 0.5, 1.0, 2.0];
/// How far one step moves paused animations on, in ms.
pub const DEFAULT_ANIMATION_STEP_MS: u32 = 100;

/// `emulation.clock` (user scope).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct ClockUserSettings {
    /// The date and time the clock shortcut sets in every view: `2026-12-24T10:00` is each
    /// page's local time, `2026-12-24T10:00:00Z` or with an offset one moment for all; empty
    /// for the moment the shortcut is pressed.
    pub default: String,
    /// How fast that clock runs: 0 holds it fixed, 1 is real time, 60 a minute per second.
    #[schemars(range(min = 0.0, max = 100.0))]
    pub speed: f64,
}

impl Default for ClockUserSettings {
    fn default() -> Self {
        Self {
            default: String::new(),
            speed: 0.0,
        }
    }
}

/// The default of `emulation.animationRates`.
pub fn default_animation_rates() -> Vec<f64> {
    DEFAULT_ANIMATION_RATES.to_vec()
}

#[cfg(test)]
#[path = "clock_settings_tests.rs"]
mod tests;
