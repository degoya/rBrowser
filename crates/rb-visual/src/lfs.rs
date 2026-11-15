//! Whether a project keeps its baselines in Git LFS (`visual.lfsCheck`): a `.gitattributes`
//! (the root's, `.rbrowser/`'s or `.rbrowser/baselines/`'s; the deepest that decides wins, as in
//! Git) gives the baseline PNGs the `lfs` filter, and `git lfs` is installed.
//! When either is missing, saving a baseline shows a notice offering the set-up; only the
//! person's own command ([`set_up`]) changes anything: it appends [`LFS_LINE`] to
//! `.gitattributes` (never through a link) and runs `git lfs install --local`. No Git command
//! that touches history runs here, and the Git runner is the app's ([`Git`]), so this crate
//! never starts a process.

use std::path::Path;

use crate::VisualError;
use crate::baseline::{read_file, replace};

/// The line the set-up appends to `.gitattributes`: the PNGs only, so `meta.json` stays text
/// a review can read.
pub const LFS_LINE: &str = ".rbrowser/baselines/**/*.png filter=lfs diff=lfs merge=lfs -text";
/// The attributes file at the project root.
pub const GITATTRIBUTES: &str = ".gitattributes";
/// The largest `.gitattributes` read.
const MAX_ATTRIBUTES_BYTES: u64 = 1024 * 1024;

/// The patterns of the root `.gitattributes` whose `filter=lfs` covers a baseline PNG.
const COVERING: &[&str] = &[
    ".rbrowser/baselines/**",
    ".rbrowser/baselines/**/*",
    ".rbrowser/baselines/**/*.png",
    ".rbrowser/baselines/*/*.png",
    ".rbrowser/**",
    "*.png",
    "**/*.png",
];
/// The same in `.rbrowser/.gitattributes` (patterns relative to `.rbrowser/`).
const COVERING_RBROWSER: &[&str] = &[
    "baselines/**",
    "baselines/**/*",
    "baselines/**/*.png",
    "baselines/*/*.png",
    "**",
    "*.png",
    "**/*.png",
];
/// The same in `.rbrowser/baselines/.gitattributes`.
const COVERING_BASELINES: &[&str] = &["**", "**/*", "**/*.png", "*/*.png", "*.png"];
/// The `.gitattributes` that can decide, root first, each with its covering patterns.
const ATTRIBUTE_FILES: [(&[&str], &[&str]); 3] = [
    (&[], COVERING),
    (&[".rbrowser"], COVERING_RBROWSER),
    (&[".rbrowser", "baselines"], COVERING_BASELINES),
];

/// The Git commands the check and the set-up need, run by the app in the project.
pub trait Git {
    /// Whether `git lfs version` succeeds in `root`.
    fn lfs_installed(&self, root: &Path) -> bool;
    /// Runs `git lfs install --local` in `root`; the reason when it failed.
    fn lfs_install_local(&self, root: &Path) -> Result<(), String>;
}

/// What the check found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LfsStatus {
    /// `.gitattributes` gives the baselines the `lfs` filter.
    pub tracked: bool,
    /// `git lfs` answers.
    pub installed: bool,
}

impl LfsStatus {
    /// Both hold: no notice.
    pub fn ok(self) -> bool {
        self.tracked && self.installed
    }
}

/// Whether `attributes` (the text of the root `.gitattributes`) gives the baselines the `lfs`
/// filter: the last line whose pattern covers them decides, as in Git.
pub fn tracks_baselines(attributes: &str) -> bool {
    decision(attributes, COVERING).unwrap_or(false)
}

/// What the last line of `attributes` with a pattern of `covering` says about the `lfs` filter;
/// `None` when no such line sets or unsets a filter.
fn decision(attributes: &str, covering: &[&str]) -> Option<bool> {
    let mut tracked = None;
    for line in attributes.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut words = line.split_whitespace();
        let Some(pattern) = words.next() else {
            continue;
        };
        if !covering.contains(&pattern.trim_start_matches('/')) {
            continue;
        }
        for attribute in words {
            match attribute {
                "filter=lfs" => tracked = Some(true),
                "-filter" | "!filter" => tracked = Some(false),
                other if other.starts_with("filter=") => tracked = Some(false),
                _ => {}
            }
        }
    }
    tracked
}

/// `attributes` with [`LFS_LINE`] appended on a line of its own.
pub fn with_lfs_line(attributes: &str) -> String {
    let mut text = attributes.to_owned();
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
    text.push_str(LFS_LINE);
    text.push('\n');
    text
}

/// The `.gitattributes` in `folder` (a link is refused); empty when missing.
fn attributes(folder: &Path) -> Result<String, VisualError> {
    let bytes = read_file(&folder.join(GITATTRIBUTES), MAX_ATTRIBUTES_BYTES)?.unwrap_or_default();
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// Whether the `.gitattributes` of the project at `root` give the baseline PNGs the `lfs`
/// filter: the deepest file that decides wins. An unreadable file counts as not tracked.
fn tracked(root: &Path) -> bool {
    let mut tracked = false;
    for (folders, covering) in ATTRIBUTE_FILES {
        let folder = folders
            .iter()
            .fold(root.to_path_buf(), |path, part| path.join(part));
        match attributes(&folder) {
            Ok(text) => {
                if let Some(decided) = decision(&text, covering) {
                    tracked = decided;
                }
            }
            Err(_) => return false,
        }
    }
    tracked
}

/// Checks the project at `root`.
pub fn check(root: &Path, git: &dyn Git) -> LfsStatus {
    LfsStatus {
        tracked: tracked(root),
        installed: git.lfs_installed(root),
    }
}

/// What the set-up did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SetUp {
    /// The line was appended (it was missing).
    pub line_added: bool,
}

/// Sets the project at `root` up for LFS baselines - the person's own command, never automatic:
/// appends [`LFS_LINE`] when the baselines are not tracked yet, then runs
/// `git lfs install --local`. Refused without `git lfs`, before anything is changed.
pub fn set_up(root: &Path, git: &dyn Git) -> Result<SetUp, VisualError> {
    if !git.lfs_installed(root) {
        return Err(VisualError::Lfs("git lfs is not installed".to_owned()));
    }
    let text = attributes(root)?;
    // A root that has the line already gets it not twice, even when a deeper file unsets it.
    let line_added = !tracked(root) && !tracks_baselines(&text);
    if line_added {
        replace(&root.join(GITATTRIBUTES), with_lfs_line(&text).as_bytes())?;
    }
    git.lfs_install_local(root).map_err(VisualError::Lfs)?;
    Ok(SetUp { line_added })
}

#[cfg(test)]
#[path = "lfs_tests.rs"]
mod tests;
