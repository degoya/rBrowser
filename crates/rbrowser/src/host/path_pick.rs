//! The window-free half of the native path dialog (RB-010-08, RB-080-19): what the dialog is
//! asked for, and the one answer the waiting page gets.
//!
//! CEF's `run_file_dialog` dereferences its `accept_filters` list without a null check (the
//! official C++ wrapper always allocates one), so a [`DialogRequest`] always carries a list, empty
//! when nothing is filtered. CEF calls the dialog callback on the UI thread, once, with no list
//! when cancelled; a callback object it releases without calling (the window closed under the
//! dialog) must still answer, so [`PickOnce`] hands the choice on exactly once, on `finish` or
//! on drop.

use std::path::PathBuf;
use std::sync::Mutex;

use rb_ipc::PathKind;

use super::ui::{PathPick, PathPicked};
use crate::cdp::lock;

/// Which native dialog to show.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DialogMode {
    /// One existing file.
    OpenFile,
    /// One existing folder.
    OpenFolder,
    /// A file to write, new or to replace (the dialog asks before replacing; RB-080-73).
    Save,
}

/// What the native dialog is asked for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DialogRequest {
    pub mode: DialogMode,
    /// The title, already translated.
    pub title: String,
    /// The accept filters (`.exe`, `image/*`); always a list, never absent.
    pub filters: Vec<String>,
    /// The folder and file name the dialog proposes (a Save dialog).
    pub default_path: Option<String>,
}

impl From<&PathPick> for DialogRequest {
    fn from(pick: &PathPick) -> Self {
        let extension = pick
            .save_as
            .as_ref()
            .and_then(|path| path.extension())
            .map(|extension| format!(".{}", extension.to_string_lossy()));
        DialogRequest {
            mode: match (pick.kind, &pick.save_as) {
                (_, Some(_)) => DialogMode::Save,
                (PathKind::File, None) => DialogMode::OpenFile,
                (PathKind::Folder, None) => DialogMode::OpenFolder,
            },
            title: pick.title.clone(),
            filters: extension.into_iter().collect(),
            default_path: pick
                .save_as
                .as_ref()
                .map(|path| path.to_string_lossy().into_owned()),
        }
    }
}

/// The first non-empty path of what the dialog returned; `None` when cancelled.
pub fn first_path(paths: impl IntoIterator<Item = String>) -> Option<PathBuf> {
    paths
        .into_iter()
        .find(|path| !path.is_empty())
        .map(PathBuf::from)
}

/// Hands the dialog's choice to the waiting [`PathPicked`] exactly once: on the first
/// [`PickOnce::finish`], or as cancelled when dropped unfinished.
pub struct PickOnce {
    done: Mutex<Option<PathPicked>>,
}

impl PickOnce {
    pub fn new(done: PathPicked) -> Self {
        PickOnce {
            done: Mutex::new(Some(done)),
        }
    }

    /// Answers with the first non-empty path of `paths` (none: cancelled); `false` when the
    /// answer was given already.
    pub fn finish(&self, paths: impl IntoIterator<Item = String>) -> bool {
        match lock(&self.done).take() {
            Some(done) => {
                done(first_path(paths));
                true
            }
            None => false,
        }
    }
}

impl Drop for PickOnce {
    fn drop(&mut self) {
        if let Some(done) = lock(&self.done).take() {
            tracing::debug!("the path dialog went away without an answer; cancelled");
            done(None);
        }
    }
}

#[cfg(test)]
#[path = "path_pick_tests.rs"]
mod tests;
