//! Installing a verified update and taking it back: the files of the switch, the journal that
//! makes it recoverable, and what a start does with an interrupted one.
//!
//! **Who does what** (the app's part arrives with RB-110-05). The app verifies and downloads the
//! artifact (`download.rs`), writes the [`Journal`] in phase `handed` and starts the updater: a
//! copy of its own executable outside the program folder ([`process::launch_updater`]), because a
//! running `.exe` cannot be replaced on Windows. Then the app ends. The updater ([`apply::run`])
//! holds [`UpdaterLock`] for its whole life, waits until no window holds [`InstanceLock`] any
//! more, switches the files, starts the new version and waits until it writes `verified` into
//! the journal at its first start. A failure takes the switch back and starts the old version;
//! a start after a crash anywhere finishes or takes back what the journal records
//! ([`recover::recover_at_start`]).
//!
//! **The portable switch** ([`portable`]) unpacks into `<install>/.update-<version>/` - next to
//! the app, on the same volume, so every move is a rename - moves each top-level entry of the new
//! archive that exists now into `<install>/.previous/`, then the new one into its place; nothing
//! else in the folder moves, and never the portable mode's `portable` marker and `data/` folder
//! ([`portable::KEPT`]). The whole folder is replaced, CEF included (no delta).
//!
//! Copied and adapted from rCommander's `crates/rc-update/src/install/` (RB-090-02): no database
//! to copy and put back, no headless server lock.

use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};

pub mod apply;
pub(crate) mod durable;
mod journal;
mod journal_check;
mod locks;
pub mod portable;
pub mod process;
pub mod recover;
pub mod steps;
pub mod trash;

pub use journal::{Journal, Phase, Plan};
pub use locks::{InstanceLock, UpdaterLock, updater_running};

/// Below the local data directory: what an update keeps between the app and the updater.
pub const UPDATE_DIR: &str = "update";
/// Inside [`UPDATE_DIR`].
pub const JOURNAL_FILE: &str = "journal.json";
/// Inside [`UPDATE_DIR`]: held by the updater for as long as it runs.
pub const LOCK_FILE: &str = "updater.lock";
/// Inside [`UPDATE_DIR`]: held, shared, by every window for as long as it runs.
pub const INSTANCE_LOCK_FILE: &str = "instance.lock";
/// Inside [`UPDATE_DIR`]: the verified downloads.
pub const DOWNLOAD_DIR: &str = "download";
/// Inside [`UPDATE_DIR`]: a journal [`Journal::check`] refused, kept for a person to read.
pub const REJECTED_FILE: &str = "journal.rejected.json";
/// Beside the program files: the entries the switch replaced.
pub const PREVIOUS_DIR: &str = ".previous";
/// How long the updater waits for the old window to end.
pub const INSTANCE_WAIT_SECS: u64 = 60;
/// How long the updater waits for the new version's proof.
pub const DEFAULT_CONFIRM_TIMEOUT_SECS: u64 = 90;
/// The hidden subcommand the updater runs as (wired with RB-110-05).
pub const APPLY_COMMAND: &str = "apply-update";
/// Honoured by a debug build only: the new version refuses to prove itself, so a test shows the
/// way back without publishing a broken release.
pub const TEST_FAIL_START_ENV: &str = "RB_UPDATE_TEST_FAIL_START";

/// `<data>/update`.
#[must_use]
pub fn update_dir(data: &Path) -> PathBuf {
    data.join(UPDATE_DIR)
}

/// Why an install was refused or failed: a stable code and what happened in words for the log.
#[derive(Debug, thiserror::Error)]
#[error("{detail}")]
pub struct InstallError {
    pub code: &'static str,
    pub detail: String,
}

impl InstallError {
    #[must_use]
    pub fn new(code: &'static str, detail: impl Into<String>) -> Self {
        Self {
            code,
            detail: detail.into(),
        }
    }
}

/// Every code an install, its updater or a start's recovery records.
pub const INSTALL_CODES: &[&str] = &[
    "update.plan_invalid",
    "update.journal_invalid",
    "update.install_dir_not_writable",
    "update.not_enough_space",
    "update.digest_mismatch",
    "update.download_failed",
    "update.archive_unknown",
    "update.archive_incomplete",
    "update.unpack_failed",
    "update.switch_failed",
    "update.start_failed",
    "update.new_version_exited",
    "update.not_confirmed",
    "update.confirm_timeout",
    "update.app_did_not_end",
    "update.interrupted",
    "update.rollback_failed",
];

/// Refuses an install that cannot succeed: a program folder this process may not write, or a
/// volume with less than twice the artifact free (`free` as the caller measured it; `None` when
/// the volume does not say, which is not refused).
///
/// # Errors
///
/// `update.install_dir_not_writable` or `update.not_enough_space`.
pub fn preflight(
    install_dir: &Path,
    artifact_size: u64,
    free: Option<u64>,
) -> Result<(), InstallError> {
    let probe = install_dir.join(".update-probe");
    let written = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&probe)
        .and_then(|mut file| file.write_all(b"probe"));
    let _ = fs::remove_file(&probe);
    if let Err(error) = written {
        return Err(InstallError::new(
            "update.install_dir_not_writable",
            format!("{} cannot be written: {error}", install_dir.display()),
        ));
    }
    let required = artifact_size.saturating_mul(2);
    match free {
        Some(free) if free < required => Err(InstallError::new(
            "update.not_enough_space",
            format!(
                "{} has {free} bytes free, the update needs {required}",
                install_dir.display()
            ),
        )),
        _ => Ok(()),
    }
}

#[cfg(test)]
pub(crate) mod fixture;

#[cfg(test)]
#[path = "install_tests.rs"]
mod tests;
