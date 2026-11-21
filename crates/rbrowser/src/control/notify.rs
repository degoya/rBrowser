//! The core's notifications to the shell and the status bar (split out of `mod.rs`, which kept
//! growing): a notification goes into the status bar's log at once and to the shell when it is
//! ready.

use std::sync::atomic::Ordering;

use rb_ipc::{CoreToShell, Notification, StatusAction, StatusCopy, event_script};

use super::Core;
use crate::cdp::lock;

impl Core {
    /// Shows `message` (a notification) in the shell, or keeps it until the shell is ready. A
    /// notification goes into the status bar's log and to the bar and its history at once: they
    /// read the log when they load, so their event never waits (RB-080-15).
    pub fn notify(&self, message: CoreToShell) {
        self.notify_with(message, None, None);
    }

    /// [`Self::notify`] with a button in the status bar that runs `action`'s command
    /// (RB-090-71); the shell's own notification carries none.
    pub fn notify_with_action(&self, notification: Notification, action: StatusAction) {
        self.notify_with(CoreToShell::Notification(notification), Some(action), None);
    }

    /// [`Self::notify_with_action`] and a button that copies a saved bug report's Markdown
    /// (RB-093-04): the status bar asks for the text and copies it itself.
    pub fn notify_with_copy(
        &self,
        notification: Notification,
        action: StatusAction,
        copy: StatusCopy,
    ) {
        self.notify_with(
            CoreToShell::Notification(notification),
            Some(action),
            Some(copy),
        );
    }

    fn notify_with(
        &self,
        mut message: CoreToShell,
        action: Option<StatusAction>,
        copy: Option<StatusCopy>,
    ) {
        if let CoreToShell::Notification(notification) = &mut message {
            // A text that names a settings path links there (RB-094-21), read while a failure's
            // code still stands in front of its sentence.
            crate::status::link_settings(notification);
            // A person reads the sentence, not the error code in front of it (RB-090-85).
            crate::status::strip_codes(notification);
            let entry = self.status.push_with_copy(
                notification.clone(),
                action,
                copy,
                crate::status::now_ms(),
            );
            match event_script(&CoreToShell::StatusMessage(entry)) {
                Ok(script) => self.ui.send_to_status_bar(script),
                Err(error) => tracing::warn!(%error, "a status message cannot be encoded"),
            }
        }
        if self.shell_ready.load(Ordering::Acquire) {
            self.send(&message);
        } else {
            lock(&self.pending).push(message);
        }
    }
}
