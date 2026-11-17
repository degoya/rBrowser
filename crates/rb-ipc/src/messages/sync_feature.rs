//! What `setSync` switches ([`SyncFeature`]), apart from `shell_to_core.rs` so that file stays
//! under the length limit; re-exported there.

use serde::{Deserialize, Serialize};

/// What `setSync` switches.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SyncFeature {
    /// Sync as a whole.
    All,
    /// Navigation.
    Navigation,
    /// Scrolling.
    Scroll,
    /// Clicks.
    Click,
    /// Text input.
    Input,
    /// Keyboard.
    Keyboard,
    /// Hover.
    Hover,
    /// Form controls' state (RB-020-05).
    Forms,
    /// Focus (RB-020-05).
    Focus,
}
