//! The status bar's messages (RB-080-15): every notification of the core, numbered and stamped
//! as the core's message log keeps it (`rbrowser::status`), some with a button that runs a
//! command (RB-090-71). Mirrored in `ui/src/ipc/notifications.ts`.

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
    /// A button that runs a command of the registry (RB-090-71).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action: Option<StatusAction>,
}

/// A status message's button: a click runs `command` with `args` through the registry, as the
/// toolbar and the palette do (`runCommand`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusAction {
    /// The command's id.
    pub command: String,
    /// The i18n key of the button's label.
    pub label_key: String,
    /// Values for the label's `{name}` placeholders.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub label_args: BTreeMap<String, String>,
    /// The command's arguments; none when `null`.
    #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
    pub args: serde_json::Value,
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
            action: None,
        }
    }

    /// The message with `action`'s button (none for `None`).
    #[must_use]
    pub fn with_action(mut self, action: Option<StatusAction>) -> Self {
        self.action = action;
        self
    }

    /// The notification it carries (without the button).
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
