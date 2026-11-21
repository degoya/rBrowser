//! The last sends of the project (RB-096-01): `.rbrowser/out/api-client/history.json`, newest
//! first, at most `api-client.historySize` (0 keeps none). An entry holds the request's URL
//! template - never a resolved value, so no secret - its saved id, the environment, the status
//! and the total time, or the failure's code. The folder is the project's output, not checked
//! in; no link on the way is followed.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// The file below the project root.
pub const HISTORY_FILE: &str = ".rbrowser/out/api-client/history.json";
/// The most entries any setting keeps.
pub const MAX_ENTRIES: usize = 500;

/// One send.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    /// Seconds since the epoch.
    pub at: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub method: String,
    /// The template, as stored.
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub environment: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_ms: Option<f64>,
    /// The failure's code, for a send that got no answer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// The file's path, when no folder on the way is a link.
fn path_below(root: &Path) -> Option<PathBuf> {
    let mut path = root.to_path_buf();
    for step in HISTORY_FILE.split('/') {
        path.push(step);
        if fs::symlink_metadata(&path).is_ok_and(|meta| meta.file_type().is_symlink()) {
            return None;
        }
    }
    Some(path)
}

/// The entries, newest first; none when there is no file or it does not read.
#[must_use]
pub fn read(root: &Path) -> Vec<Entry> {
    path_below(root)
        .and_then(|path| fs::read(path).ok())
        .and_then(|bytes| serde_json::from_slice::<Vec<Entry>>(&bytes).ok())
        .map(|mut entries| {
            entries.truncate(MAX_ENTRIES);
            entries
        })
        .unwrap_or_default()
}

/// Puts `entry` first and keeps `keep` entries; `keep` 0 removes the file.
///
/// # Errors
///
/// A link on the way, or the write failed.
pub fn append(root: &Path, entry: Entry, keep: usize) -> std::io::Result<()> {
    let path = path_below(root).ok_or_else(|| std::io::Error::other("a link on the way"))?;
    if keep == 0 {
        return match fs::remove_file(&path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        };
    }
    let mut entries = read(root);
    entries.insert(0, entry);
    entries.truncate(keep.min(MAX_ENTRIES));
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_vec_pretty(&entries).map_err(std::io::Error::other)?;
    rb_util::fs::write_atomic(&path, &text)
}

#[cfg(test)]
#[path = "history_tests.rs"]
mod tests;
