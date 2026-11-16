//! The status bar's messages (RB-080-15): every notification of the core, numbered and stamped
//! as the core's message log keeps it (`rbrowser::status`). Mirrored in `ui/src/ipc/protocol.ts`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::payloads::{Notification, NotificationLevel};

/// A notification as the status bar shows it and its history lists it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusMessage {
    /// Its number in the window's log, rising from 1.
    pub id: u64,
    /// When the core took it, in milliseconds since the Unix epoch.
    pub at: u64,
    /// Severity.
    pub level: NotificationLevel,
    /// The i18n key of the text.
    pub message_key: String,
    /// Values for the text's `{name}` placeholders.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub args: BTreeMap<String, String>,
    /// A setting to link to (opens `rbrowser://settings/<key>`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settings_key: Option<String>,
}

impl StatusMessage {
    /// `notification` as message `id`, taken at `at` (ms since the Unix epoch).
    #[must_use]
    pub fn new(id: u64, at: u64, notification: Notification) -> Self {
        Self {
            id,
            at,
            level: notification.level,
            message_key: notification.message_key,
            args: notification.args,
            settings_key: notification.settings_key,
        }
    }

    /// The notification it carries.
    #[must_use]
    pub fn notification(&self) -> Notification {
        Notification {
            level: self.level,
            message_key: self.message_key.clone(),
            args: self.args.clone(),
            settings_key: self.settings_key.clone(),
        }
    }
}

#[cfg(test)]
#[path = "status_tests.rs"]
mod tests;
