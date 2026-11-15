//! The watcher's state for the toolbar (watching, polling, error) and `project_info`.

use serde::Serialize;

use super::classify::HotMode;

/// How the project is watched.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(
    tag = "state",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum WatcherStatus {
    /// Not watching: no project, or `reload.mode` is off.
    Off,
    /// Starting (the WSL helper's handshake, the first walk).
    Starting,
    /// File events of the system (ReadDirectoryChangesW, inotify).
    Native {
        /// The folders watched (1 for a recursive watch).
        folders: u64,
    },
    /// `rbrowser-wsl watch` inside the distribution.
    Helper {
        /// The distribution.
        distro: String,
        /// The folders watched.
        folders: u64,
    },
    /// Polling: asked for (`reload.pollingFallback`) or the fallback.
    Polling {
        /// The folders polled.
        folders: u64,
        /// Why it is the fallback (the i18n key of the failure); `None` when asked for.
        #[serde(skip_serializing_if = "Option::is_none")]
        reason_key: Option<String>,
    },
    /// No watch.
    Failed {
        /// The i18n key of the reason.
        reason_key: String,
        /// The reason for the log.
        detail: String,
    },
}

impl WatcherStatus {
    /// The i18n key of the toolbar's label.
    #[must_use]
    pub fn label_key(&self) -> &'static str {
        match self {
            Self::Off => "reload.status.off",
            Self::Starting => "reload.status.starting",
            Self::Native { .. } => "reload.status.watching",
            Self::Helper { .. } => "reload.status.helper",
            Self::Polling { .. } => "reload.status.polling",
            Self::Failed { .. } => "reload.status.failed",
        }
    }
}

/// The watcher in `project_info` (RB-030-01) and the toolbar.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WatcherInfo {
    /// How the project is watched.
    pub status: WatcherStatus,
    /// The toolbar's label.
    pub label_key: &'static str,
    /// Hot reload paused for this session (`reload.toggle_pause`).
    pub paused: bool,
    /// `reload.mode`: `auto`, `full` or `off`.
    pub mode: &'static str,
    /// The interval reload in effect, in seconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interval_sec: Option<u64>,
}

impl WatcherInfo {
    /// The info for `status` and the rest.
    #[must_use]
    pub fn new(
        status: WatcherStatus,
        paused: bool,
        mode: HotMode,
        interval_sec: Option<u64>,
    ) -> Self {
        Self {
            label_key: if paused {
                "reload.status.paused"
            } else {
                status.label_key()
            },
            status,
            paused,
            mode: match mode {
                HotMode::Auto => "auto",
                HotMode::Full => "full",
                HotMode::Off => "off",
            },
            interval_sec,
        }
    }
}

#[cfg(test)]
#[path = "status_tests.rs"]
mod tests;
