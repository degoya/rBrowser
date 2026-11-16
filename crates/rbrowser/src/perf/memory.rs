//! The memory warning (RB-020-09), pure: rBrowser's processes together above
//! `performance.memoryWarningMb` raise one warning in the shell; the next comes after the use
//! fell below nine tenths of the threshold, or at once when the threshold changes.

use std::collections::BTreeMap;

use rb_ipc::{CoreToShell, Notification, NotificationLevel};

/// The i18n key of the warning (`{usedMb}`, `{limitMb}`).
pub const MEMORY_WARNING_KEY: &str = "perf.memoryWarning";
/// The setting the warning links to.
pub const MEMORY_WARNING_SETTING: &str = "performance.memoryWarningMb";

const MIB: u64 = 1024 * 1024;

/// One warning: the memory in use and the threshold, MiB.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MemoryWarning {
    /// In use.
    pub used_mb: u64,
    /// The threshold.
    pub limit_mb: u32,
}

/// Whether a warning is due.
#[derive(Clone, Copy, Debug, Default)]
pub struct MemoryWatch {
    /// The threshold a warning was given at and not re-armed since.
    warned_at: Option<u32>,
}

impl MemoryWatch {
    /// `used_bytes` are in use under the threshold `limit_mb` (0: off); the warning when due.
    pub fn observe(&mut self, used_bytes: u64, limit_mb: u32) -> Option<MemoryWarning> {
        if limit_mb == 0 {
            self.warned_at = None;
            return None;
        }
        let used_mb = used_bytes / MIB;
        let limit = u64::from(limit_mb);
        if self.warned_at.is_some_and(|at| at != limit_mb) || used_mb * 10 < limit * 9 {
            self.warned_at = None;
        }
        if used_mb <= limit || self.warned_at.is_some() {
            return None;
        }
        self.warned_at = Some(limit_mb);
        Some(MemoryWarning { used_mb, limit_mb })
    }
}

/// How far the process count moves before it gets another line in the log.
pub const COUNT_STEP: u32 = 20;

/// When rBrowser's process count is worth a line in the log (RB-080-67): at the first sample,
/// then whenever it has moved by [`COUNT_STEP`] since the last line, up or down - a leak shows up
/// while it grows, not only once the memory warning comes.
#[derive(Clone, Copy, Debug, Default)]
pub struct CountWatch {
    logged: Option<u32>,
}

impl CountWatch {
    /// `processes` run now; whether to log them.
    pub fn observe(&mut self, processes: u32) -> bool {
        if self
            .logged
            .is_some_and(|logged| logged.abs_diff(processes) < COUNT_STEP)
        {
            return false;
        }
        self.logged = Some(processes);
        true
    }
}

/// The i18n key of the warning that a helper process runs untied (`{count}`).
pub const UNTIED_KEY: &str = "perf.processTreeUntied";

/// Whether to say that helpers run outside their job object (rb-process, review 9 of
/// RB-080-67): once per run of the app, at the first one.
#[derive(Clone, Copy, Debug, Default)]
pub struct UntiedWatch {
    said: bool,
}

impl UntiedWatch {
    /// `untied` helpers run untied so far; the count to say, once.
    pub fn observe(&mut self, untied: u32) -> Option<u32> {
        if self.said || untied == 0 {
            return None;
        }
        self.said = true;
        Some(untied)
    }
}

/// The shell's warning that `count` helpers run outside their job object: their processes may
/// outlive them.
pub fn untied_notification(count: u32) -> CoreToShell {
    CoreToShell::Notification(Notification {
        level: NotificationLevel::Warning,
        message_key: UNTIED_KEY.to_owned(),
        args: BTreeMap::from([("count".to_owned(), count.to_string())]),
        settings_key: None,
    })
}

/// The shell's warning for `warning`, linking the setting.
pub fn notification(warning: MemoryWarning) -> CoreToShell {
    CoreToShell::Notification(Notification {
        level: NotificationLevel::Warning,
        message_key: MEMORY_WARNING_KEY.to_owned(),
        args: BTreeMap::from([
            ("usedMb".to_owned(), warning.used_mb.to_string()),
            ("limitMb".to_owned(), warning.limit_mb.to_string()),
        ]),
        settings_key: Some(MEMORY_WARNING_SETTING.to_owned()),
    })
}

#[cfg(test)]
#[path = "memory_tests.rs"]
mod tests;
