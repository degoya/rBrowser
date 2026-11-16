//! Watching a project folder for hot reload (RB-030-05): the rules, the batching and the tree
//! of watches, shared by the helper inside WSL (`rbrowser-wsl watch`, inotify) and the app's own
//! watcher for projects on a Windows drive (ReadDirectoryChangesW, or polling).
//!
//! - [`ignore`]: gitignore-style rules: the defaults (`node_modules`, `.git`, dot folders, editor
//!   leftovers), `reload.ignore` and `.rbrowser/ignore`.
//! - [`filter`]: the rules plus the `reload.watch` globs: which files count, which folders are
//!   walked at all.
//! - [`batch`]: the debounce: changes within the quiet time become one batch.
//! - [`tree`]: the watches on a folder tree, ignored folders left out; notify events made into
//!   relative [`Change`]s.
//! - [`run`]: the helper's `watch` loop over the stdio protocol.
//!
//! Paths in a [`Change`] are relative to the watched root and `/` separated on every platform.

pub mod batch;
pub mod filter;
pub mod ignore;
pub mod run;
pub mod tree;

use serde::{Deserialize, Serialize};

/// What happened to a path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeKind {
    /// Created.
    Create,
    /// Written to, or its metadata changed.
    Modify,
    /// Deleted.
    Remove,
    /// Renamed (from or to this name).
    Rename,
}

/// One changed path below the watched root.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Change {
    /// Relative to the root, `/` separated (`src/app.css`).
    pub path: String,
    /// What happened last within the batch.
    pub kind: ChangeKind,
}

impl Change {
    /// A change of `path`.
    #[must_use]
    pub fn new(path: impl Into<String>, kind: ChangeKind) -> Self {
        Self {
            path: path.into(),
            kind,
        }
    }
}

/// The rules of a watch, as rBrowser sends them to the helper (`watch` message) and hands them
/// to its own watcher. Built from the project config (`reload.watch`, `reload.ignore`,
/// `reload.debounceMs`) and `.rbrowser/ignore`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WatchRules {
    /// Globs relative to the root; a file must match one. Empty: every file that is not ignored.
    #[serde(default)]
    pub watch: Vec<String>,
    /// Gitignore lines, applied after [`ignore::DEFAULT_IGNORES`].
    #[serde(default)]
    pub ignore: Vec<String>,
    /// The quiet time after the last change before a batch goes out.
    pub debounce_ms: u64,
}

impl Default for WatchRules {
    fn default() -> Self {
        Self {
            watch: Vec::new(),
            ignore: Vec::new(),
            debounce_ms: batch::DEFAULT_DEBOUNCE_MS,
        }
    }
}

/// `path` (below `root`) relative to it and `/` separated; `None` outside of it or for the root
/// itself.
#[must_use]
pub fn relative(root: &std::path::Path, path: &std::path::Path) -> Option<String> {
    let rest = path.strip_prefix(root).ok()?;
    let parts: Vec<&str> = rest
        .components()
        .map(|part| part.as_os_str().to_str())
        .collect::<Option<_>>()?;
    if parts.is_empty() {
        return None;
    }
    Some(parts.join("/"))
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
