//! A person's flow export (RB-090-66): "Save" in the Recorder panel's export dialog puts the
//! flow where flows live - `<automation.flows.dir>/<name>.ts`, `.rbrowser/flows/` by default -
//! so the Flows panel lists it. An existing flow is replaced only when the person said so in the
//! dialog.
//!
//! `.rbrowser/flows/` is a protected folder of workspace trust (K9, threat model T1): a new file
//! there changes the hash of the protected parts, and the project would ask for trust again. The
//! person's click on "Save" is the trust decision for exactly that one file: when the project
//! was trusted before the write, and after it its protected parts are the same entries plus this
//! file's ([`only_this_changed`]), the decision is moved to the new hash - under the trust
//! store's lock, and only while the store still holds the decision the check started from. Any
//! other difference, or a project that was not trusted, is left to the existing prompt.
//!
//! Only the shell's panel request (`exportRecording` with `write`, [`super::panel`]) reaches this
//! module; MCP's `recorder_export` writes into `automation.recorderExport.dir` and never here.

use std::collections::BTreeSet;
use std::fs::OpenOptions;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use rb_config::trust::{
    BlockedItem, ProtectedKind, TrustState, load_trust_store, trust_state, update_trust_store,
};

pub use crate::flows::catalog::PROTECTED_DIR as DEFAULT_FLOWS_DIR;
use crate::flows::catalog::{EXTENSION, flows_dir, relative, valid_name};

/// Why a flow was not written.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum FlowFileError {
    /// Not a flow name.
    #[error("{0:?} is no flow name (letters, digits, - _ and . only)")]
    BadName(String),
    /// `automation.flows.dir` leaves `.rbrowser/flows`.
    #[error("automation.flows.dir {0:?} must be .rbrowser/flows or a folder inside it")]
    BadDir(String),
    /// A flow of that name exists, and the person did not say to replace it.
    #[error("the flow {0} exists already")]
    Exists(String),
    /// A folder on the way to the flow, or the flow file itself, is a link: a repository could
    /// point it out of the project.
    #[error("{0} is a link; a flow is only written into the project's own folders")]
    Linked(String),
    /// The file could not be written.
    #[error("the flow could not be written: {0}")]
    Io(String),
}

/// Where a flow goes and against which trust store.
#[derive(Clone, Copy, Debug)]
pub struct Target<'a> {
    /// The project root.
    pub root: &'a Path,
    /// `automation.flows.dir`.
    pub dir: &'a str,
    /// The flow's name, its file name without `.ts`.
    pub name: &'a str,
    /// The trust store (`trust.json`); without one nothing is trusted.
    pub store: Option<&'a Path>,
    /// Now, RFC 3339 UTC, for the moved decision.
    pub now: &'a str,
}

/// A flow written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SavedFlow {
    /// Relative to the project root, with `/`.
    pub path: String,
    /// The project's trust covers the new file: it was trusted before and nothing else changed.
    pub trust_kept: bool,
}

/// The file of `target`'s flow.
///
/// # Errors
///
/// [`FlowFileError::BadName`] or [`FlowFileError::BadDir`].
pub fn path_of(target: &Target<'_>) -> Result<PathBuf, FlowFileError> {
    if !valid_name(target.name) {
        return Err(FlowFileError::BadName(target.name.to_owned()));
    }
    let dir = flows_dir(target.root, target.dir)
        .map_err(|_| FlowFileError::BadDir(target.dir.to_owned()))?;
    Ok(dir.join(format!("{}.{EXTENSION}", target.name)))
}

/// Writes `source` as `target`'s flow - a new file, or over an existing one with `overwrite` -
/// and keeps the project's trust for exactly that file when it had it.
///
/// # Errors
///
/// A bad name or folder, an existing flow without `overwrite`, or the file cannot be written.
pub fn write_flow(
    target: &Target<'_>,
    source: &str,
    overwrite: bool,
) -> Result<SavedFlow, FlowFileError> {
    let path = path_of(target)?;
    let shown = relative(target.root, &path);
    let before = trusted_state(target);
    refuse_links(target.root, &path)?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|error| FlowFileError::Io(error.to_string()))?;
    }
    // Again once the folders exist: one could have been made a link in between.
    refuse_links(target.root, &path)?;
    let mut options = OpenOptions::new();
    options.write(true);
    if overwrite {
        options.create(true).truncate(true);
    } else {
        options.create_new(true);
    }
    let mut file = options.open(&path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::AlreadyExists {
            FlowFileError::Exists(shown.clone())
        } else {
            FlowFileError::Io(error.to_string())
        }
    })?;
    file.write_all(source.as_bytes())
        .and_then(|()| file.sync_all())
        .map_err(|error| FlowFileError::Io(error.to_string()))?;
    drop(file);
    tracing::info!(bytes = source.len(), "flow saved from the recorder");
    let trust_kept = match (before, target.store) {
        (Some(before), Some(store)) => keep_trust(target, store, &before, |after| {
            only_this_changed(&before, after, &shown)
        }),
        _ => false,
    };
    Ok(SavedFlow {
        path: shown,
        trust_kept,
    })
}

/// [`FlowFileError::Linked`] when anything between `root` and `path` - `.rbrowser`, the flows
/// folder, a subfolder or the file - is a link (a symlink or, on Windows, a junction); the root
/// itself may be one.
pub(crate) fn refuse_links(root: &Path, path: &Path) -> Result<(), FlowFileError> {
    let below = path
        .strip_prefix(root)
        .map_err(|_| FlowFileError::Linked(path.display().to_string()))?;
    let mut at = root.to_path_buf();
    for part in below.components() {
        at.push(part);
        if std::fs::symlink_metadata(&at).is_ok_and(|meta| meta.file_type().is_symlink()) {
            return Err(FlowFileError::Linked(relative(root, &at)));
        }
    }
    Ok(())
}

/// The project's trust state when it is trusted now.
pub(crate) fn trusted_state(target: &Target<'_>) -> Option<TrustState> {
    let store = load_trust_store(target.store?).ok()?;
    trust_state(target.root, &store)
        .ok()
        .filter(|state| state.trusted)
}

/// Moves the decision of `before` to the hash after the person's change - a flow saved, or
/// deleted (RB-090-91) - when `only_that` holds for the protected parts now (nothing else
/// changed); `false` leaves the prompt to the existing check.
pub(crate) fn keep_trust(
    target: &Target<'_>,
    store: &Path,
    before: &TrustState,
    only_that: impl FnOnce(&TrustState) -> bool,
) -> bool {
    let Ok(after) = load_trust_store(store).and_then(|loaded| trust_state(target.root, &loaded))
    else {
        return false;
    };
    if !only_that(&after) {
        tracing::info!("other protected parts changed beside the flow; trust asks again");
        return false;
    }
    let mut moved = false;
    let updated = update_trust_store(store, |decisions| {
        if decisions
            .record(target.root)
            .is_some_and(|record| record.hash == before.hash)
        {
            decisions.grant(target.root, &after.hash, target.now);
            moved = true;
        }
    });
    if let Err(error) = updated {
        tracing::warn!(%error, "the trust store could not be updated for the flow");
        return false;
    }
    moved
}

/// Whether the protected parts `after` are those of `before` plus the entry of the file
/// `written` (relative to the root) - the same entries when it replaced one - with no link the
/// hash cannot pin.
#[must_use]
pub fn only_this_changed(before: &TrustState, after: &TrustState, written: &str) -> bool {
    let entries = |state: &TrustState| -> BTreeSet<(ProtectedKind, String)> {
        state
            .blocked
            .iter()
            .map(|BlockedItem { kind, path }| (*kind, path.clone()))
            .collect()
    };
    let mut expected = entries(before);
    expected.insert((ProtectedKind::Flows, top_level_entry(written)));
    after.unpinned.is_empty() && entries(after) == expected
}

/// Whether the protected parts `after` are those of `before` without the entry of the file
/// `removed` (relative to the root) - the same entries when its subfolder stays - with no link
/// the hash cannot pin: the mirror of [`only_this_changed`] for a deleted flow (RB-090-91).
#[must_use]
pub fn only_this_removed(before: &TrustState, after: &TrustState, removed: &str) -> bool {
    after.unpinned.is_empty() && only_this_changed(after, before, removed)
}

/// The entry of the flows folder that holds `written`: the file itself, or the subfolder of
/// `.rbrowser/flows/` it lies in - what the trust state lists.
#[must_use]
pub fn top_level_entry(written: &str) -> String {
    let prefix = format!("{DEFAULT_FLOWS_DIR}/");
    match written.strip_prefix(&prefix) {
        Some(rest) => match rest.split_once('/') {
            Some((first, _)) => format!("{prefix}{first}"),
            None => written.to_owned(),
        },
        None => written.to_owned(),
    }
}

#[cfg(test)]
#[path = "flow_file_tests.rs"]
mod tests;
