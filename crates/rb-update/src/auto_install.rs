//! Installing a downloaded update by itself (`updates.autoInstall`, RB-090-14): when, not how.
//!
//! Off by default. `onQuit` installs a downloaded offer when rBrowser quits and leaves the new
//! version to the next start a person makes, which proves it. Quitting is never held up: an
//! offer still downloading is left for the next run, and so is one while other windows run (the
//! last of them to quit installs it; RB-090-14). Never by itself: an installation that does
//! not install itself, or a version whose install failed or was taken back before. The caller
//! observes; this decides.
//!
//! Adapted from rCommander's `crates/rc-update/src/auto_install.rs`: no idle mode, no job queue.

use serde::Serialize;

/// The setting `updates.autoInstall`.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum AutoMode {
    #[default]
    Off,
    OnQuit,
}

/// What the app sees when it quits.
#[derive(Clone, Copy, Debug, Default)]
pub struct Moment<'a> {
    pub mode: AutoMode,
    /// The installation updates itself (`InstallKind::installs_itself`).
    pub installs_itself: bool,
    /// The version offered and completely downloaded and verified, if any.
    pub ready: Option<&'a str>,
    /// The install of that version failed or was taken back before.
    pub failed_before: bool,
    /// Other rBrowser windows (other projects) still run: the updater would wait for them in
    /// vain, so the last window to quit installs.
    pub other_windows: usize,
}

/// Why quitting installs nothing.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Wait {
    Off,
    Unsupported,
    NothingReady,
    FailedBefore,
    OtherWindows,
}

impl Wait {
    /// The name `update_status` answers.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Unsupported => "unsupported",
            Self::NothingReady => "nothing_ready",
            Self::FailedBefore => "failed_before",
            Self::OtherWindows => "other_windows",
        }
    }
}

/// Whether quitting installs the ready offer.
///
/// # Errors
///
/// Why not.
pub fn on_quit(moment: &Moment<'_>) -> Result<(), Wait> {
    if moment.mode == AutoMode::Off {
        return Err(Wait::Off);
    }
    if !moment.installs_itself {
        return Err(Wait::Unsupported);
    }
    if moment.ready.is_none() {
        return Err(Wait::NothingReady);
    }
    if moment.failed_before {
        return Err(Wait::FailedBefore);
    }
    if moment.other_windows > 0 {
        return Err(Wait::OtherWindows);
    }
    Ok(())
}

#[cfg(test)]
#[path = "auto_install_tests.rs"]
mod tests;
