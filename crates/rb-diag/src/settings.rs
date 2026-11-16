//! The settings of logging and diagnostics as the [`LogSettings`] struct the app reads from the
//! user settings.
//!
//! Every key is a user setting (`%APPDATA%\rBrowser\settings.json`), writable over MCP
//! (`settings_set`): Advanced > Log level and Diagnostics, Security & Privacy > Logs, General >
//! Updates. The keys, their schema and their place in the settings window are defined once, in
//! rb-config's user settings (`logs`, `updates.cefAgeWarning`); the settings registry is built
//! from that schema, and the app's tests hold these defaults to it.

use serde::{Deserialize, Serialize};

use crate::rotate::RotationPolicy;

/// `logs.level`: the level of the log file (`RUST_LOG`, when set, wins).
pub const KEY_LEVEL: &str = "logs.level";
/// `logs.maxFileSizeMb`: the size at which the log file rotates.
pub const KEY_MAX_FILE_SIZE: &str = "logs.maxFileSizeMb";
/// `logs.maxFiles`: the log files kept, the current one included.
pub const KEY_MAX_FILES: &str = "logs.maxFiles";
/// `logs.crashDumpsInBundle`: the newest crash dumps the diagnostics ZIP takes (0 = none).
pub const KEY_CRASH_DUMPS: &str = "logs.crashDumpsInBundle";
/// `updates.cefAgeWarning`: the notice about a CEF build older than six weeks.
pub const KEY_CEF_AGE_WARNING: &str = "updates.cefAgeWarning";

/// Default of [`KEY_MAX_FILE_SIZE`], in MiB.
pub const DEFAULT_MAX_FILE_SIZE_MB: u64 = 10;
/// Default of [`KEY_MAX_FILES`].
pub const DEFAULT_MAX_FILES: usize = 5;
/// Default of [`KEY_CRASH_DUMPS`].
pub const DEFAULT_CRASH_DUMPS: usize = 3;

/// A log level, lowest volume first. Serialized lowercase, as `RUST_LOG` writes it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    /// Errors only.
    Error,
    /// Warnings and errors.
    Warn,
    /// The default.
    #[default]
    Info,
    /// Detail for a bug report.
    Debug,
    /// Everything.
    Trace,
}

impl LogLevel {
    /// Every level, in the order of the settings' drop-down.
    pub const ALL: [LogLevel; 5] = [
        LogLevel::Error,
        LogLevel::Warn,
        LogLevel::Info,
        LogLevel::Debug,
        LogLevel::Trace,
    ];

    /// The name `RUST_LOG` and the settings use.
    pub fn as_str(self) -> &'static str {
        match self {
            LogLevel::Error => "error",
            LogLevel::Warn => "warn",
            LogLevel::Info => "info",
            LogLevel::Debug => "debug",
            LogLevel::Trace => "trace",
        }
    }
}

/// The `logs` object of the user settings.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct LogSettings {
    /// [`KEY_LEVEL`].
    pub level: LogLevel,
    /// [`KEY_MAX_FILE_SIZE`].
    pub max_file_size_mb: u64,
    /// [`KEY_MAX_FILES`].
    pub max_files: usize,
    /// [`KEY_CRASH_DUMPS`].
    pub crash_dumps_in_bundle: usize,
}

impl Default for LogSettings {
    fn default() -> Self {
        Self {
            level: LogLevel::default(),
            max_file_size_mb: DEFAULT_MAX_FILE_SIZE_MB,
            max_files: DEFAULT_MAX_FILES,
            crash_dumps_in_bundle: DEFAULT_CRASH_DUMPS,
        }
    }
}

impl LogSettings {
    /// The rotation these settings ask for.
    pub fn rotation(&self) -> RotationPolicy {
        RotationPolicy::from_settings(self.max_file_size_mb, self.max_files)
    }
}

#[cfg(test)]
#[path = "settings_tests.rs"]
mod tests;
