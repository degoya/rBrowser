//! The message log of the status bar (RB-080-15): the window's last messages, numbered from 1
//! and stamped, the oldest dropped once it holds `ui.statusBar.historySize`, and whether the bar
//! is shown; and the check of a message the shell reports itself.

use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use rb_config::StatusBarSettings;
use rb_ipc::{Notification, NotificationLevel, StatusAction, StatusMessage};

use crate::cdp::lock;

/// The longest message key or settings key a report may name.
pub const MAX_KEY_CHARS: usize = 200;
/// The most placeholder values a report may carry.
pub const MAX_ARGS: usize = 16;
/// The longest placeholder value a report keeps; a longer one is cut.
pub const MAX_ARG_CHARS: usize = 2000;

/// The window's messages, oldest first. Any thread.
#[derive(Debug)]
pub struct StatusLog {
    inner: Mutex<Inner>,
}

#[derive(Debug)]
struct Inner {
    last_id: u64,
    capacity: usize,
    items: VecDeque<StatusMessage>,
    shown: bool,
}

impl StatusLog {
    /// An empty log that keeps `capacity` messages (at least one).
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            inner: Mutex::new(Inner {
                last_id: 0,
                capacity: capacity.max(1),
                items: VecDeque::new(),
                shown: true,
            }),
        }
    }

    /// An empty log as `settings` say: its capacity and whether the bar is shown.
    #[must_use]
    pub fn for_settings(settings: &StatusBarSettings) -> Self {
        let log = Self::new(settings.history_capacity());
        log.set_shown(settings.visible);
        log
    }

    /// Notes whether the bar is shown; `true` when that changed.
    pub fn set_shown(&self, shown: bool) -> bool {
        let mut inner = lock(&self.inner);
        let changed = inner.shown != shown;
        inner.shown = shown;
        changed
    }

    /// Takes `notification` as the next message, at `at` (ms since the Unix epoch); a placeholder
    /// value longer than [`MAX_ARG_CHARS`] is cut, as in a report.
    pub fn push(&self, notification: Notification, at: u64) -> StatusMessage {
        self.push_with(notification, None, at)
    }

    /// [`Self::push`] with a button that runs `action`'s command (RB-090-71).
    pub fn push_with(
        &self,
        mut notification: Notification,
        action: Option<StatusAction>,
        at: u64,
    ) -> StatusMessage {
        cut_values(&mut notification);
        let mut inner = lock(&self.inner);
        inner.last_id += 1;
        let message = StatusMessage::new(inner.last_id, at, notification).with_action(action);
        inner.items.push_back(message.clone());
        inner.trim();
        message
    }

    /// Every message kept, oldest first.
    #[must_use]
    pub fn items(&self) -> Vec<StatusMessage> {
        lock(&self.inner).items.iter().cloned().collect()
    }

    /// The newest `limit` messages, newest first; only those of `level` when given.
    #[must_use]
    pub fn recent(&self, limit: usize, level: Option<NotificationLevel>) -> Vec<StatusMessage> {
        lock(&self.inner)
            .items
            .iter()
            .rev()
            .filter(|message| level.is_none_or(|level| message.level == level))
            .take(limit)
            .cloned()
            .collect()
    }

    /// Keeps `capacity` messages from now on (at least one), dropping the oldest.
    pub fn set_capacity(&self, capacity: usize) {
        let mut inner = lock(&self.inner);
        inner.capacity = capacity.max(1);
        inner.trim();
    }
}

impl Inner {
    fn trim(&mut self) {
        while self.items.len() > self.capacity {
            self.items.pop_front();
        }
    }
}

/// Now, in milliseconds since the Unix epoch (0 before it).
#[must_use]
pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| {
            u64::try_from(since.as_millis()).unwrap_or(u64::MAX)
        })
}

/// A message the shell reports (`reportStatus`) as the log takes it: its keys and placeholder
/// names are dotted ASCII names, it carries at most [`MAX_ARGS`] values, and a value longer than
/// [`MAX_ARG_CHARS`] is cut.
///
/// # Errors
///
/// What is wrong with it, in English.
pub fn checked_report(report: &Notification) -> Result<Notification, String> {
    if !is_name(&report.message_key) {
        return Err(
            "`messageKey` must be a dotted name of letters, digits, `_` and `-`".to_owned(),
        );
    }
    if report
        .settings_key
        .as_deref()
        .is_some_and(|key| !is_name(key))
    {
        return Err(
            "`settingsKey` must be a dotted name of letters, digits, `_` and `-`".to_owned(),
        );
    }
    if report.args.len() > MAX_ARGS {
        return Err(format!("a report carries at most {MAX_ARGS} values"));
    }
    if let Some(name) = report.args.keys().find(|name| !is_name(name)) {
        return Err(format!("`{name}` is not a placeholder name"));
    }
    let mut checked = report.clone();
    cut_values(&mut checked);
    Ok(checked)
}

/// Cuts every placeholder value of `notification` longer than [`MAX_ARG_CHARS`].
fn cut_values(notification: &mut Notification) {
    for value in notification.args.values_mut() {
        if let Some((end, _)) = value.char_indices().nth(MAX_ARG_CHARS) {
            value.truncate(end);
            value.push_str("...");
        }
    }
}

fn is_name(text: &str) -> bool {
    !text.is_empty()
        && text.chars().count() <= MAX_KEY_CHARS
        && text
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

#[cfg(test)]
#[path = "log_tests.rs"]
mod tests;
