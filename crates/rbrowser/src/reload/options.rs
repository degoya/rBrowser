//! The settings of hot reload in one place: the project's `reload` (watch, ignore, debounce,
//! mode, interval, HMR), the environments' own intervals and the user's (polling).

use std::collections::BTreeMap;
use std::time::Duration;

use rb_config::{Environment, ReloadConfig, ReloadMode, UserReloadSettings};
use rb_wsl::watch::WatchRules;
use rb_wsl::watch::ignore::parse_ignore_file;

use super::classify::HotMode;

/// The shortest poll: a poll walks every watched folder.
pub const MIN_POLL_INTERVAL_MS: u64 = 250;
/// The shortest interval reload.
pub const MIN_INTERVAL_SEC: u64 = 2;

/// Hot reload's settings in effect.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReloadOptions {
    /// `reload.mode`.
    pub mode: HotMode,
    /// `reload.watch`.
    pub watch: Vec<String>,
    /// `reload.ignore`.
    pub ignore: Vec<String>,
    /// `reload.debounceMs`.
    pub debounce_ms: u64,
    /// `reload.respectHmr`: a page with its own HMR client (Vite, webpack) is left to it.
    pub respect_hmr: bool,
    /// `reload.intervalSec`: reload every view this often; 0 is off.
    pub interval_sec: u64,
    /// `environments[].reloadIntervalSec`: the interval of an environment, over the global one.
    pub environment_intervals: BTreeMap<String, u64>,
    /// User `reload.pollingFallback`: poll instead of file events.
    pub polling: bool,
    /// User `reload.pollIntervalMs`.
    pub poll_interval_ms: u64,
    /// User `reload.intervalPauseSec`.
    pub interval_pause_sec: u64,
}

impl Default for ReloadOptions {
    fn default() -> Self {
        Self::from_config(
            &ReloadConfig::default(),
            &[],
            &UserReloadSettings::default(),
        )
    }
}

impl ReloadOptions {
    /// The options of the project's `reload`, its environments and the user's `reload`.
    #[must_use]
    pub fn from_config(
        project: &ReloadConfig,
        environments: &[Environment],
        user: &UserReloadSettings,
    ) -> Self {
        Self {
            mode: hot_mode(project.mode),
            watch: project.watch.clone(),
            ignore: project.ignore.clone(),
            debounce_ms: project.debounce_ms,
            respect_hmr: project.respect_hmr,
            interval_sec: project.interval_sec,
            environment_intervals: environments
                .iter()
                .filter_map(|env| Some((env.id.clone(), env.reload_interval_sec?)))
                .collect(),
            polling: user.polling_fallback,
            poll_interval_ms: user.poll_interval_ms,
            interval_pause_sec: user.interval_pause_sec,
        }
    }

    /// The watch rules with the lines of the project's ignore file (`.rbrowser/ignore`).
    #[must_use]
    pub fn rules(&self, ignore_file: Option<&str>) -> WatchRules {
        let mut ignore = self.ignore.clone();
        if let Some(text) = ignore_file {
            ignore.extend(parse_ignore_file(text));
        }
        WatchRules {
            watch: self.watch.clone(),
            ignore,
            debounce_ms: self.debounce_ms,
        }
    }

    /// How often polling looks, at least [`MIN_POLL_INTERVAL_MS`].
    #[must_use]
    pub fn poll_interval(&self) -> Duration {
        Duration::from_millis(self.poll_interval_ms.max(MIN_POLL_INTERVAL_MS))
    }

    /// The interval reload for the environment `environment` (its own, else the global one);
    /// `None` when off. At least [`MIN_INTERVAL_SEC`].
    #[must_use]
    pub fn interval(&self, environment: Option<&str>) -> Option<Duration> {
        let seconds = environment
            .and_then(|id| self.environment_intervals.get(id).copied())
            .unwrap_or(self.interval_sec);
        if seconds == 0 {
            return None;
        }
        Some(Duration::from_secs(seconds.max(MIN_INTERVAL_SEC)))
    }

    /// How long input pauses the interval reload.
    #[must_use]
    pub fn interval_pause(&self) -> Duration {
        Duration::from_secs(self.interval_pause_sec)
    }
}

/// `reload.mode` of the config. `manual` is the job's `off`.
#[must_use]
pub fn hot_mode(mode: ReloadMode) -> HotMode {
    match mode {
        ReloadMode::Auto => HotMode::Auto,
        ReloadMode::Full => HotMode::Full,
        ReloadMode::Manual => HotMode::Off,
    }
}

/// The most of `.rbrowser/ignore` read; a longer file is cut (rules are lines, not data).
pub const MAX_IGNORE_FILE: u64 = 256 * 1024;

/// The project's ignore file at `root`, when there is one.
#[must_use]
pub fn read_ignore_file(root: &std::path::Path) -> Option<String> {
    use std::io::Read as _;
    let path = root.join(rb_wsl::watch::ignore::IGNORE_FILE);
    let file = std::fs::File::open(path).ok()?;
    let mut text = String::new();
    file.take(MAX_IGNORE_FILE)
        .read_to_string(&mut text)
        .inspect_err(|error| tracing::warn!(%error, "the project's ignore file cannot be read"))
        .ok()?;
    Some(text)
}

#[cfg(test)]
#[path = "options_tests.rs"]
mod tests;
