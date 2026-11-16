//! The watches on a project tree and what their events mean.
//!
//! [`TreeMode::PerFolder`] (inotify, polling) watches every folder the [`Filter`] walks, one by
//! one, so `node_modules` costs no inotify watch and no poll; a folder created later is walked
//! and watched when its event arrives. [`TreeMode::Recursive`] (ReadDirectoryChangesW: one handle
//! covers the tree) watches the root alone and drops the events of ignored folders.
//!
//! Generic over the notify [`Watcher`], so the helper (inotify), the app (ReadDirectoryChangesW)
//! and the polling fallback share it.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use notify::event::{EventKind, ModifyKind};
use notify::{ErrorKind, Event, RecursiveMode, Watcher};

use super::filter::Filter;
use super::{Change, ChangeKind, relative};

/// The most folders a [`TreeMode::PerFolder`] watch takes; a tree with more is refused with
/// [`TreeError::TooManyFolders`] (narrow it with `.rbrowser/ignore`).
pub const MAX_FOLDERS: usize = 50_000;

/// How the tree is watched.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreeMode {
    /// Every walked folder on its own (inotify, polling).
    PerFolder,
    /// The root, recursively (ReadDirectoryChangesW).
    Recursive,
}

/// Why a tree cannot be watched.
#[derive(Debug, thiserror::Error)]
pub enum TreeError {
    /// The root is no folder, or cannot be read.
    #[error("the folder {0} cannot be watched: {1}")]
    Root(String, String),
    /// More than [`MAX_FOLDERS`] folders.
    #[error("the project has more than {MAX_FOLDERS} folders to watch")]
    TooManyFolders,
    /// The system's limit of watches (inotify `max_user_watches`).
    #[error("the system limit of file watches is reached")]
    WatchLimit,
}

impl TreeError {
    /// The i18n key of the message a user reads.
    #[must_use]
    pub fn i18n_key(&self) -> &'static str {
        match self {
            Self::Root(..) => "reload.error.root",
            Self::TooManyFolders => "reload.error.tooManyFolders",
            Self::WatchLimit => "reload.error.watchLimit",
        }
    }
}

/// What an event means.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Seen {
    /// A path that counts changed.
    Change(Change),
    /// Events were lost (inotify queue overflow): something changed, unknown what.
    Rescan,
}

/// The watches on one tree.
#[derive(Debug)]
pub struct TreeWatch<W: Watcher> {
    root: PathBuf,
    filter: Filter,
    watcher: W,
    mode: TreeMode,
    watched: BTreeSet<PathBuf>,
}

impl<W: Watcher> TreeWatch<W> {
    /// Watches the tree at `root` with `watcher`, whose events the caller feeds to
    /// [`Self::on_event`].
    ///
    /// # Errors
    ///
    /// [`TreeError`] when the root cannot be watched or the tree is too large.
    pub fn new(root: &Path, filter: Filter, watcher: W, mode: TreeMode) -> Result<Self, TreeError> {
        if !root.is_dir() {
            return Err(TreeError::Root(
                root.display().to_string(),
                "not a folder".into(),
            ));
        }
        let mut tree = Self {
            root: root.to_path_buf(),
            filter,
            watcher,
            mode,
            watched: BTreeSet::new(),
        };
        match mode {
            TreeMode::Recursive => tree.add(root, RecursiveMode::Recursive)?,
            TreeMode::PerFolder => tree.walk(root)?,
        }
        Ok(tree)
    }

    /// The root.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The folders watched (one with [`TreeMode::Recursive`]).
    #[must_use]
    pub fn folders(&self) -> usize {
        self.watched.len()
    }

    /// Whether `folder` is watched.
    #[must_use]
    pub fn watches(&self, folder: &Path) -> bool {
        self.watched.contains(folder)
    }

    /// Walks the tree again after lost events: folders created meanwhile are watched.
    ///
    /// # Errors
    ///
    /// [`TreeError`] as [`Self::new`].
    pub fn rewalk(&mut self) -> Result<(), TreeError> {
        match self.mode {
            TreeMode::Recursive => Ok(()),
            TreeMode::PerFolder => {
                let root = self.root.clone();
                self.walk(&root)
            }
        }
    }

    /// What `event` means; a folder it creates is watched now.
    ///
    /// # Errors
    ///
    /// [`TreeError`] when a new folder would pass a limit.
    pub fn on_event(&mut self, event: &Event) -> Result<Vec<Seen>, TreeError> {
        if event.need_rescan() {
            return Ok(vec![Seen::Rescan]);
        }
        let kind = match event.kind {
            EventKind::Create(_) => ChangeKind::Create,
            EventKind::Modify(ModifyKind::Name(_)) => ChangeKind::Rename,
            EventKind::Modify(_) | EventKind::Any | EventKind::Other => ChangeKind::Modify,
            EventKind::Remove(_) => ChangeKind::Remove,
            EventKind::Access(_) => return Ok(Vec::new()),
        };
        let mut seen = Vec::new();
        for path in &event.paths {
            let Some(rel) = relative(&self.root, path) else {
                continue;
            };
            // Not followed: a folder link made at runtime is no folder of the project, so its
            // target is never walked (`walk` skips links the same way).
            let is_dir = std::fs::symlink_metadata(path).is_ok_and(|meta| meta.is_dir());
            if matches!(kind, ChangeKind::Remove | ChangeKind::Rename) && !is_dir {
                self.forget(path);
            }
            if is_dir
                && self.mode == TreeMode::PerFolder
                && matches!(kind, ChangeKind::Create | ChangeKind::Rename)
                && self.filter.walks(&rel, true)
            {
                self.walk(path)?;
            }
            // A folder reported as modified (ReadDirectoryChangesW does when an entry in it
            // changes, inotify when its attributes do) changes no page: the entry's own event
            // reports what changed.
            let folder_touched = is_dir && kind == ChangeKind::Modify;
            if !folder_touched && self.filter.accepts(&rel, is_dir) {
                seen.push(Seen::Change(Change::new(rel, kind)));
            }
        }
        Ok(seen)
    }

    /// Watches `start` and every folder below it that the filter walks.
    fn walk(&mut self, start: &Path) -> Result<(), TreeError> {
        let mut stack = vec![start.to_path_buf()];
        while let Some(folder) = stack.pop() {
            if !self.watched.contains(&folder) {
                if self.watched.len() >= MAX_FOLDERS {
                    return Err(TreeError::TooManyFolders);
                }
                match self.add(&folder, RecursiveMode::NonRecursive) {
                    Ok(()) => {}
                    Err(TreeError::Root(..)) if folder != self.root => continue,
                    Err(error) => return Err(error),
                }
            }
            let Ok(entries) = std::fs::read_dir(&folder) else {
                tracing::debug!(folder = %folder.display(), "a folder cannot be read");
                continue;
            };
            for entry in entries.flatten() {
                // `file_type` does not follow links: a linked folder is not walked (no loops).
                if !entry.file_type().is_ok_and(|kind| kind.is_dir()) {
                    continue;
                }
                let path = entry.path();
                if relative(&self.root, &path).is_some_and(|rel| self.filter.walks(&rel, true))
                    && !self.watched.contains(&path)
                {
                    stack.push(path);
                }
            }
        }
        Ok(())
    }

    fn add(&mut self, folder: &Path, mode: RecursiveMode) -> Result<(), TreeError> {
        match self.watcher.watch(folder, mode) {
            Ok(()) => {
                self.watched.insert(folder.to_path_buf());
                Ok(())
            }
            Err(error) if matches!(error.kind, ErrorKind::MaxFilesWatch) => {
                Err(TreeError::WatchLimit)
            }
            Err(error) => Err(TreeError::Root(
                folder.display().to_string(),
                error.to_string(),
            )),
        }
    }

    /// Drops the watches of a folder that is gone (removed or renamed away) and those below it.
    fn forget(&mut self, path: &Path) {
        // Paths order by component, so a folder's descendants follow it in one run.
        let gone: Vec<PathBuf> = self
            .watched
            .range(path.to_path_buf()..)
            .take_while(|folder| folder.starts_with(path))
            .filter(|folder| folder.as_path() != self.root)
            .cloned()
            .collect();
        for folder in gone {
            // inotify drops the watch of a deleted folder itself; nothing to undo then.
            let _ = self.watcher.unwatch(&folder);
            self.watched.remove(&folder);
        }
    }
}

#[cfg(test)]
#[path = "tree_tests.rs"]
mod tests;
