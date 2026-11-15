//! Where logs, crash dumps and diagnostics ZIPs live.
//!
//! Windows: `%LOCALAPPDATA%\rBrowser\` with `logs\` (`rbrowser.log`, `cef.log`), `diagnostics\`
//! (the ZIPs) and `User Data\Crashpad\` - the crash database CEF derives from `AppName=rBrowser`
//! in `crash_reporter.cfg` (`cef`). Elsewhere (the Linux build and tests) the same layout under
//! `$XDG_STATE_HOME/rBrowser` or `~/.local/state/rBrowser`.
//!
//! The app builds its paths with [`DiagPaths::under`] from `rb_config::local_dir()`, which is
//! `<exe dir>\data\local` in the portable mode (a file `portable` beside `rbrowser.exe`,
//! RB-010-15); [`DiagPaths::from_env`] knows only the installed layout.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// The folder of the app under the local app data.
pub const APP_DIR: &str = "rBrowser";
/// The core's log file.
pub const LOG_FILE_NAME: &str = "rbrowser.log";
/// CEF's log file (`CefSettings.log_file`), beside the core's.
pub const CEF_LOG_FILE_NAME: &str = "cef.log";

/// The directories of the diagnostics.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiagPaths {
    /// The log files: ours and CEF's.
    pub logs: PathBuf,
    /// The Crashpad database CEF writes its dumps into (`reports\*.dmp`).
    pub crash_dumps: PathBuf,
    /// Where the diagnostics ZIPs go when no `--out` is given.
    pub bundles: PathBuf,
}

impl DiagPaths {
    /// The layout under `root` (`%LOCALAPPDATA%\rBrowser`).
    pub fn under(root: &Path) -> Self {
        Self {
            logs: root.join("logs"),
            crash_dumps: root.join("User Data").join("Crashpad"),
            bundles: root.join("diagnostics"),
        }
    }

    /// The paths of this machine; `None` when neither `%LOCALAPPDATA%` (Windows) nor
    /// `$XDG_STATE_HOME`/`$HOME` (elsewhere) is set.
    pub fn from_env() -> Option<Self> {
        Self::resolve(cfg!(windows), |name| std::env::var_os(name))
    }

    /// [`DiagPaths::from_env`] with the platform and the environment given, for tests.
    pub fn resolve(windows: bool, env: impl Fn(&str) -> Option<OsString>) -> Option<Self> {
        let set = |name: &str| {
            env(name)
                .filter(|value| !value.is_empty())
                .map(PathBuf::from)
        };
        let root = if windows {
            set("LOCALAPPDATA")?
        } else {
            set("XDG_STATE_HOME").or_else(|| set("HOME").map(|home| home.join(".local/state")))?
        };
        Some(Self::under(&root.join(APP_DIR)))
    }

    /// The core's log file.
    pub fn log_file(&self) -> PathBuf {
        self.logs.join(LOG_FILE_NAME)
    }

    /// CEF's log file.
    pub fn cef_log_file(&self) -> PathBuf {
        self.logs.join(CEF_LOG_FILE_NAME)
    }
}

#[cfg(test)]
#[path = "paths_tests.rs"]
mod tests;
