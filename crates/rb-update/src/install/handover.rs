//! The app's half of an install (RB-090-14): the plan from a verified download and the running
//! program, the journal in phase `handed`, and the updater started - then the app ends, at once
//! for "Install and restart" or when a person quits for `updates.autoInstall = onQuit`.
//!
//! The order holds what `apply.rs` assumes: the plan is valid and the shipped updater is there
//! before a journal is written, and no second updater runs; a journal is written before the
//! updater starts, so a start that fails leaves `handed`, which the next start of the app ends as
//! `update.interrupted` (`recover.rs`).

use std::path::{Path, PathBuf};

use super::{
    DEFAULT_CONFIRM_TIMEOUT_SECS, DOWNLOAD_DIR, InstallError, Journal, Phase, Plan, process,
    update_dir, updater_running,
};
use crate::install_kind::InstallKind;
use crate::manifest::Artifact;

/// The program that runs now.
#[derive(Clone, Debug)]
pub struct Running {
    /// Its executable, absolute (`std::env::current_exe`).
    pub executable: PathBuf,
    /// Its version.
    pub version: String,
    /// How it was installed.
    pub kind: InstallKind,
    /// The local data folder.
    pub data_dir: PathBuf,
    /// Its own arguments, so the new version starts as it did.
    pub args: Vec<String>,
}

/// `<data>/update/download`: where verified downloads wait.
#[must_use]
pub fn download_dir(data: &Path) -> PathBuf {
    update_dir(data).join(DOWNLOAD_DIR)
}

/// The plan to switch `running` to `target_version` from the verified `downloaded` file of
/// `artifact`; `start_new` false for "install when quitting".
///
/// # Errors
///
/// `update.plan_invalid` for a program without a folder or a name, and every refusal of
/// [`Plan::validate`].
pub fn plan(
    running: &Running,
    target_version: &str,
    artifact: &Artifact,
    downloaded: &Path,
    start_new: bool,
) -> Result<Plan, InstallError> {
    let invalid = |detail: String| InstallError::new("update.plan_invalid", detail);
    let install_dir = running
        .executable
        .parent()
        .ok_or_else(|| invalid(format!("{} has no folder", running.executable.display())))?;
    let executable = running
        .executable
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| invalid(format!("{} has no name", running.executable.display())))?;
    let plan = Plan {
        kind: running.kind,
        from_version: running.version.clone(),
        target_version: target_version.to_owned(),
        artifact: downloaded.to_path_buf(),
        sha256: artifact.sha256.clone(),
        size: artifact.size,
        install_dir: install_dir.to_path_buf(),
        executable: executable.to_owned(),
        data_dir: running.data_dir.clone(),
        args: running.args.clone(),
        start_new,
        confirm_timeout_secs: DEFAULT_CONFIRM_TIMEOUT_SECS,
    };
    plan.validate()?;
    Ok(plan)
}

/// Writes the journal for `plan` and starts the updater; the caller ends the app next.
///
/// # Errors
///
/// `update.plan_invalid`, `update.updater_missing` (a package without `rbrowser-updater`),
/// `update.busy` (an updater runs), `update.failed` when the journal cannot be written, and
/// `update.start_failed` when the updater does not start - the journal then says `handed`.
pub fn hand_over(plan: Plan) -> Result<(), InstallError> {
    plan.validate()?;
    let updater = process::shipped_updater(&plan.install_dir);
    if !updater.is_file() {
        return Err(InstallError::new(
            "update.updater_missing",
            format!("{} is missing", updater.display()),
        ));
    }
    if updater_running(&plan.data_dir) {
        return Err(InstallError::new(
            "update.busy",
            "an update is being installed already",
        ));
    }
    let mut journal = Journal::begin(plan);
    journal
        .write()
        .map_err(|error| InstallError::new("update.failed", format!("{error:#}")))?;
    process::launch_updater(&journal)
        .map_err(|error| InstallError::new("update.start_failed", format!("{error:#}")))
}

/// Removes every verified download of `data` but those in `keep` and the one the journal names
/// (an update in progress, or one the manual recovery may want); a `.partial` file is another
/// window's download in progress and stays. Returns how many went.
pub fn prune_downloads(data: &Path, keep: &[&Path]) -> usize {
    let journal = Journal::read(data)
        .ok()
        .flatten()
        .map(|journal| journal.plan.artifact);
    let Ok(entries) = std::fs::read_dir(download_dir(data)) else {
        return 0;
    };
    let mut removed = 0;
    for path in entries.flatten().map(|entry| entry.path()) {
        let partial = path
            .extension()
            .is_some_and(|extension| extension == "partial");
        let kept = keep.iter().any(|kept| *kept == path) || journal.as_ref() == Some(&path);
        if partial || kept || !path.is_file() {
            continue;
        }
        match std::fs::remove_file(&path) {
            Ok(()) => removed += 1,
            Err(error) => {
                tracing::warn!(%error, "a superseded update download stays until a later start");
            }
        }
    }
    removed
}

/// Whether the last update of `data` was to `version` and failed or was taken back: such a
/// version is never installed by itself again, only by a person's click.
#[must_use]
pub fn failed_before(data: &Path, version: &str) -> bool {
    Journal::read(data).ok().flatten().is_some_and(|journal| {
        journal.plan.target_version == version
            && matches!(journal.phase, Phase::RolledBack | Phase::Failed)
    })
}

#[cfg(test)]
#[path = "handover_tests.rs"]
mod tests;
