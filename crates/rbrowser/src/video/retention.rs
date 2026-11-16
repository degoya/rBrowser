//! The retention of the Videos panel (RB-110-12): after each saved recording the oldest beyond
//! `capture.video.keepRecordings` and those older than `capture.video.keepDays` are deleted - only
//! recordings rBrowser wrote ([`super::registry`]), never the one just made nor one being written.
//! A recording whose files went but whose folder stayed is kept as pending in the registry and its
//! folder removed on a later run ([`settle_pending`]).

use std::path::Path;
use std::time::{Duration, SystemTime};

use super::library::{self, LibraryError, RecordingEntry};
use super::registry::Registry;

/// A day.
const DAY: Duration = Duration::from_secs(86_400);

/// The ids of `recordings` (newest first) the retention deletes at `now`: those after the
/// newest `keep` and those older than `keep_days` days; 0 turns a limit off. The ids in
/// `protect` (the recording just made, one being written) are never among them.
#[must_use]
pub fn expired(
    recordings: &[RecordingEntry],
    keep: u32,
    keep_days: u32,
    now: SystemTime,
    protect: &[&str],
) -> Vec<String> {
    let max_age = DAY.saturating_mul(keep_days);
    let keep = usize::try_from(keep).unwrap_or(usize::MAX);
    recordings
        .iter()
        .enumerate()
        .filter(|(_, recording)| !protect.contains(&recording.id.as_str()))
        .filter(|(index, recording)| {
            let beyond_count = keep > 0 && *index >= keep;
            let too_old = keep_days > 0
                && recording
                    .modified()
                    .and_then(|modified| now.duration_since(modified).ok())
                    .is_some_and(|age| age > max_age);
            beyond_count || too_old
        })
        .map(|(_, recording)| recording.id.clone())
        .collect()
}

/// Applies the retention to the project at `root` after a recording: deletes the [`expired`]
/// recordings rBrowser wrote, forgets them in `registry` and answers their ids. A recording
/// that cannot be deleted is logged and stays.
///
/// # Errors
///
/// The recordings cannot be listed.
pub fn apply(
    root: &Path,
    registry: &Registry,
    keep: u32,
    keep_days: u32,
    now: SystemTime,
    protect: &[&str],
) -> Result<Vec<String>, LibraryError> {
    settle_pending(root, registry);
    if keep == 0 && keep_days == 0 {
        return Ok(Vec::new());
    }
    let recorded = registry.load();
    let recordings = library::list(root, &recorded)?;
    let mut deleted = Vec::new();
    for id in expired(&recordings, keep, keep_days, now, protect) {
        match library::delete(root, &recorded, &id, None, protect) {
            Ok(removed) => {
                if let Err(error) = registry.forget_deleted(&id, None, removed.folder_removed) {
                    tracing::warn!(%error, "the video registry was not updated");
                }
                deleted.push(id);
            }
            Err(error) => {
                tracing::warn!(%error, recording = %id, "an old recording was not deleted");
            }
        }
    }
    Ok(deleted)
}

/// Removes the folders of the deleted recordings that stayed behind (a file a player held open
/// on Windows) once they are empty, and drops them from the pending list; answers the ids
/// cleared. Runs before every retention and every list of the panel.
pub fn settle_pending(root: &Path, registry: &Registry) -> Vec<String> {
    let recorded = registry.load();
    let cleared: Vec<String> = recorded
        .pending
        .iter()
        .filter(|id| library::remove_empty_folder(root, id))
        .cloned()
        .collect();
    if !cleared.is_empty()
        && let Err(error) = registry.clear_pending(&cleared)
    {
        tracing::warn!(%error, "the video registry was not updated");
    }
    cleared
}

#[cfg(test)]
#[path = "retention_tests.rs"]
mod tests;
