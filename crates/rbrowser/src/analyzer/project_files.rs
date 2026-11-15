//! The project files of the analyser's static source (RB-030-07): the host reads them for the
//! plugin and hands their text over, so the plugin never touches the disk.
//!
//! A file is read only when both the plugin's declared `project-read` globs (`granted`, the upper
//! bound in its signed manifest) and `analyzer.staticGlobs` (which can only narrow it) match its
//! path relative to the project root. The walk stays inside the root: it starts there, takes the
//! names `read_dir` gives (never `..`), follows no link (a symlink or junction is skipped, so
//! nothing outside the project is reached) and never enters `node_modules`, a dot folder
//! (`.git`, `.rbrowser` - so never `config.local.json`) or a path `.rbrowser/ignore` names
//! (gitignore-style lines; `!` negations are not supported). It reads only, within the limits
//! below. A WSL project's root is its UNC path (`\\wsl.localhost\<distro>\...`), which `std::fs`
//! reads like any other folder.

use std::path::{Path, PathBuf};

use globset::{Glob, GlobBuilder, GlobSet, GlobSetBuilder};
use serde::Serialize;

/// The most files handed over.
pub const MAX_FILES: usize = 256;
/// The largest file read.
pub const MAX_FILE_BYTES: u64 = 1024 * 1024;
/// The most text handed over in one analysis.
pub const MAX_TOTAL_BYTES: usize = 16 * 1024 * 1024;
/// The deepest folder entered below the root.
pub const MAX_DEPTH: usize = 16;
/// The most folder entries looked at.
pub const MAX_ENTRIES: usize = 50_000;
/// The most globs taken from `analyzer.staticGlobs` and lines from `.rbrowser/ignore`.
pub const MAX_PATTERNS: usize = 64;
/// The longest glob or ignore line taken.
const MAX_PATTERN_CHARS: usize = 256;
/// The project's ignore file, relative to the root.
pub const IGNORE_FILE: [&str; 2] = [".rbrowser", "ignore"];
/// The largest ignore file read.
const MAX_IGNORE_BYTES: u64 = 64 * 1024;

/// One project file, as the plugin's `detect` takes it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ProjectFile {
    /// Relative to the project root, `/`-separated.
    pub path: String,
    /// Its text (UTF-8; a file that is not is skipped).
    pub text: String,
}

/// Whether a glob from the config is one this reader takes: relative, `/`-separated, without
/// `..`, a drive or a control character, and not too long.
fn acceptable(glob: &str) -> bool {
    !glob.is_empty()
        && glob.chars().count() <= MAX_PATTERN_CHARS
        && !glob.starts_with('/')
        && !glob.contains(['\\', ':'])
        && !glob.chars().any(char::is_control)
        && !glob.split('/').any(|segment| segment == "..")
}

/// The globs of `analyzer.staticGlobs` that are acceptable and compile; others are logged and
/// left out.
fn wanted(globs: &[String]) -> Option<GlobSet> {
    let mut builder = GlobSetBuilder::new();
    let mut any = false;
    for glob in globs.iter().take(MAX_PATTERNS) {
        match Glob::new(glob) {
            Ok(compiled) if acceptable(glob) => {
                builder.add(compiled);
                any = true;
            }
            _ => tracing::warn!(glob = %glob, "an analyzer.staticGlobs entry was left out"),
        }
    }
    if !any {
        return None;
    }
    builder.build().ok()
}

/// The rules of `.rbrowser/ignore`: a line without `/` (but a trailing one) matches the name at
/// any depth, one with `/` from the root; a match leaves out the path and all below it.
fn ignore_rules(root: &Path) -> GlobSet {
    let path: PathBuf = IGNORE_FILE
        .iter()
        .fold(root.to_path_buf(), |path, part| path.join(part));
    let text = match std::fs::symlink_metadata(&path) {
        Ok(metadata) if metadata.is_file() && metadata.len() <= MAX_IGNORE_BYTES => {
            std::fs::read_to_string(&path).unwrap_or_default()
        }
        _ => String::new(),
    };
    let mut builder = GlobSetBuilder::new();
    for line in text.lines().take(MAX_PATTERNS * 4) {
        let line = line.trim();
        if line.is_empty()
            || line.starts_with('#')
            || line.starts_with('!')
            || line.chars().count() > MAX_PATTERN_CHARS
        {
            continue;
        }
        let pattern = line.trim_end_matches('/');
        let anchored = pattern.starts_with('/') || pattern.contains('/');
        let pattern = pattern.trim_start_matches('/');
        if pattern.is_empty() {
            continue;
        }
        let glob = if anchored {
            pattern.to_owned()
        } else {
            format!("**/{pattern}")
        };
        for glob in [glob.clone(), format!("{glob}/**")] {
            if let Ok(compiled) = GlobBuilder::new(&glob).literal_separator(true).build() {
                builder.add(compiled);
            }
        }
    }
    builder.build().unwrap_or_else(|_| GlobSet::empty())
}

/// Reads the project files below `root` that `granted` (the plugin's declared globs) and `globs`
/// (`analyzer.staticGlobs`) both match, in a stable order (by path, folder by folder).
pub fn collect(root: &Path, globs: &[String], granted: &dyn Fn(&str) -> bool) -> Vec<ProjectFile> {
    let Some(wanted) = wanted(globs) else {
        return Vec::new();
    };
    let ignored = ignore_rules(root);
    let mut walk = Walk {
        files: Vec::new(),
        total_bytes: 0,
        entries: 0,
    };
    let mut folders = vec![(root.to_path_buf(), String::new(), 0_usize)];
    while let Some((folder, relative, depth)) = folders.pop() {
        let Ok(reader) = std::fs::read_dir(&folder) else {
            continue;
        };
        // Counted while listing: a folder of a million entries is not listed whole first.
        let budget = MAX_ENTRIES.saturating_sub(walk.entries).saturating_add(1);
        let mut entries: Vec<(String, std::fs::DirEntry)> = reader
            .flatten()
            .take(budget)
            .filter_map(|entry| Some((entry.file_name().into_string().ok()?, entry)))
            .collect();
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        // Folders are taken from the stack last-in first-out: push them in reverse.
        let mut below = Vec::new();
        for (name, entry) in entries {
            walk.entries += 1;
            if walk.entries > MAX_ENTRIES || walk.files.len() >= MAX_FILES {
                tracing::warn!("the static source stopped at its limits; some files were not read");
                return walk.files;
            }
            // The type of the entry itself: a link is never followed.
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if name.starts_with('.') || kind.is_symlink() {
                continue;
            }
            let path = if relative.is_empty() {
                name.clone()
            } else {
                format!("{relative}/{name}")
            };
            if ignored.is_match(&path) {
                continue;
            }
            if kind.is_dir() {
                if name != "node_modules" && depth + 1 < MAX_DEPTH {
                    below.push((entry.path(), path, depth + 1));
                }
            } else if kind.is_file() && wanted.is_match(&path) && granted(&path) {
                walk.read(&entry.path(), path);
            }
        }
        folders.extend(below.into_iter().rev());
    }
    walk.files
}

struct Walk {
    files: Vec<ProjectFile>,
    total_bytes: usize,
    entries: usize,
}

impl Walk {
    fn read(&mut self, file: &Path, path: String) {
        let Some(text) = read_regular(file, MAX_FILE_BYTES) else {
            tracing::debug!(path = %path, "a project file was skipped: over the limit, no text, or no longer a plain file");
            return;
        };
        if self.total_bytes + text.len() > MAX_TOTAL_BYTES {
            tracing::warn!(path = %path, "the static source read its limit of project files");
            return;
        }
        self.total_bytes += text.len();
        self.files.push(ProjectFile { path, text });
    }
}

/// The text of the plain file at `path`, at most `limit` bytes; `None` for a larger file, one
/// that is not UTF-8, or anything that is no plain file when it is opened - a link swapped in
/// after the walk looked is not followed (Windows opens the link itself, elsewhere the handle
/// must be the file the walk saw). Size and type come from the open handle, never the path.
fn read_regular(path: &Path, limit: u64) -> Option<String> {
    use std::io::Read;
    let seen = std::fs::symlink_metadata(path).ok()?;
    if !seen.is_file() {
        return None;
    }
    let file = no_follow::open(path).ok()?;
    let opened = file.metadata().ok()?;
    if !opened.is_file() || !no_follow::same_file(&seen, &opened) || opened.len() > limit {
        return None;
    }
    let mut bytes = Vec::new();
    file.take(limit.saturating_add(1))
        .read_to_end(&mut bytes)
        .ok()?;
    if !u64::try_from(bytes.len()).is_ok_and(|read| read <= limit) {
        return None;
    }
    String::from_utf8(bytes).ok()
}

/// Opening a file without following a link in its last component.
mod no_follow {
    use std::fs::{File, Metadata, OpenOptions};
    use std::path::Path;

    /// `FILE_FLAG_OPEN_REPARSE_POINT`: a symlink or junction opens as itself.
    #[cfg(windows)]
    const OPEN_REPARSE_POINT: u32 = 0x0020_0000;

    #[cfg(windows)]
    pub fn open(path: &Path) -> std::io::Result<File> {
        use std::os::windows::fs::OpenOptionsExt;
        OpenOptions::new()
            .read(true)
            .custom_flags(OPEN_REPARSE_POINT)
            .open(path)
    }

    #[cfg(not(windows))]
    pub fn open(path: &Path) -> std::io::Result<File> {
        OpenOptions::new().read(true).open(path)
    }

    /// Windows opened the link itself, so the handle's type says it all.
    #[cfg(windows)]
    pub fn same_file(_seen: &Metadata, _opened: &Metadata) -> bool {
        true
    }

    /// The handle is the file the walk saw (device and inode), not a link's target.
    #[cfg(unix)]
    pub fn same_file(seen: &Metadata, opened: &Metadata) -> bool {
        use std::os::unix::fs::MetadataExt;
        seen.dev() == opened.dev() && seen.ino() == opened.ino()
    }

    #[cfg(not(any(windows, unix)))]
    pub fn same_file(_seen: &Metadata, _opened: &Metadata) -> bool {
        true
    }
}

#[cfg(test)]
#[path = "project_files_tests.rs"]
mod tests;
