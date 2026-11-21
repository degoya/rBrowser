//! `reload` of the user settings (RB-030-05): polling instead of file events and the interval
//! reload's pause - split from `user_settings.rs` (500-line rule), re-exported there.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// `reload` of the user settings.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct UserReloadSettings {
    /// Poll instead of relying on file events.
    pub polling_fallback: bool,
    /// How often polling looks for changes, in milliseconds (RB-030-05).
    #[schemars(range(min = 250))]
    pub poll_interval_ms: u64,
    /// How long input in a view pauses the interval reload, in seconds (RB-030-05).
    pub interval_pause_sec: u64,
}

impl Default for UserReloadSettings {
    fn default() -> Self {
        Self {
            polling_fallback: false,
            poll_interval_ms: 1000,
            interval_pause_sec: 10,
        }
    }
}
