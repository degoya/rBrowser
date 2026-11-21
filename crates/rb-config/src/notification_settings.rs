//! `ui.notifications` of the user settings (RB-094-20): how a message reaches the person beside
//! the status bar, which stays the log of every message. Settings > Layout > Status bar.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// `ui.notifications`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct NotificationSettings {
    /// The error of an action you started - adding a device, starting a view, a menu row or a
    /// toolbar button that failed - shows at the top of the window as well, in the toolbar's
    /// strip, never over the views, with the message's buttons (Settings, Retry, Download).
    /// Off: the status bar alone.
    pub error_toast: bool,
}

impl Default for NotificationSettings {
    fn default() -> Self {
        Self { error_toast: true }
    }
}

#[cfg(test)]
#[path = "notification_settings_tests.rs"]
mod tests;
