//! The path rule of `project-read`, `project-write` and `out-write`: what a plugin may name, before the host looks
//! at the disk. Shared by the host and the native test host, so a plugin's native test refuses
//! exactly what the host will.
//!
//! New in rBrowser (rCommander's plugins see handles, never paths). A plugin names a path relative
//! to its area - the project root for `project-read`, `.rbrowser/out/<plugin-id>/` for
//! `out-write` - and the host refuses anything that could leave it: `..`, an absolute path, a
//! drive or a stream (`:`), a Windows device name, a segment Windows would silently change (a
//! trailing dot or space) or a short name it made up (`~` and a digit). Names are compared the
//! way Windows does, whatever their case ([`same_name`]). `/` and `\` both separate, since Windows reads either. What the disk
//! adds (a link pointing out) is the host's second check, after the path is resolved.

use crate::{Failure, FailureKind, codes};

/// The project's config folder.
pub const CONFIG_DIR: &str = ".rbrowser";
/// The folder under [`CONFIG_DIR`] each plugin writes its own folder of.
pub const OUT_DIR: &str = "out";
/// The machine-local config `project-read` never hands out: it may hold tokens.
pub const LOCAL_CONFIG_FILE: &str = "config.local.json";
/// The version-control folder `project-write` never writes into, at any depth.
pub const VCS_DIR: &str = ".git";
/// The longest path a plugin may name, in characters.
pub const MAX_PATH_CHARS: usize = 1024;
/// The most segments a plugin's path may have.
pub const MAX_SEGMENTS: usize = 32;

// With the console names and the superscript ports Windows also reserves (security review
// RB-110-13).
const DEVICE_NAMES: [&str; 30] = [
    "con",
    "prn",
    "aux",
    "nul",
    "com1",
    "com2",
    "com3",
    "com4",
    "com5",
    "com6",
    "com7",
    "com8",
    "com9",
    "lpt1",
    "lpt2",
    "lpt3",
    "lpt4",
    "lpt5",
    "lpt6",
    "lpt7",
    "lpt8",
    "lpt9",
    "conin$",
    "conout$",
    "com\u{b9}",
    "com\u{b2}",
    "com\u{b3}",
    "lpt\u{b9}",
    "lpt\u{b2}",
    "lpt\u{b3}",
];

/// Why a path was refused.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum PathRefusal {
    #[error("the path is empty")]
    Empty,
    #[error("the path is too long")]
    TooLong,
    #[error("the path is absolute or names a drive")]
    Absolute,
    #[error("the path climbs out with `..`")]
    Parent,
    #[error("the path has an empty, `.` or device segment, or one Windows would change")]
    Segment,
    #[error("the path holds a control character")]
    Control,
}

impl PathRefusal {
    /// The failure a plugin gets: `plugins.error.path_outside` naming the path it sent.
    #[must_use]
    pub fn failure(self, path: &str) -> Failure {
        let shown: String = path.chars().take(200).collect();
        Failure::coded(FailureKind::AccessDenied, codes::PATH_OUTSIDE).with_param("path", shown)
    }
}

/// The segments of `path`, a path relative to the plugin's area, or why it is refused.
///
/// # Errors
///
/// Every [`PathRefusal`] case; see the module documentation.
pub fn relative_segments(path: &str) -> Result<Vec<&str>, PathRefusal> {
    if path.is_empty() {
        return Err(PathRefusal::Empty);
    }
    if path.chars().count() > MAX_PATH_CHARS {
        return Err(PathRefusal::TooLong);
    }
    if path.chars().any(char::is_control) {
        return Err(PathRefusal::Control);
    }
    if path.starts_with(['/', '\\']) || path.contains(':') {
        return Err(PathRefusal::Absolute);
    }
    let segments: Vec<&str> = path.split(['/', '\\']).collect();
    if segments.len() > MAX_SEGMENTS {
        return Err(PathRefusal::TooLong);
    }
    for segment in &segments {
        if *segment == ".." {
            return Err(PathRefusal::Parent);
        }
        if !segment_allowed(segment) {
            return Err(PathRefusal::Segment);
        }
    }
    Ok(segments)
}

fn segment_allowed(segment: &str) -> bool {
    if segment.is_empty() || segment == "." || segment.ends_with(['.', ' ']) {
        return false;
    }
    // `NAME~1.EXT`: a short name Windows makes up opens the long-named file, whose name the
    // rules below never see (review finding M3 of Charge 6).
    if segment
        .as_bytes()
        .windows(2)
        .any(|pair| pair[0] == b'~' && pair[1].is_ascii_digit())
    {
        return false;
    }
    let stem = segment
        .split('.')
        .next()
        .unwrap_or(segment)
        .to_ascii_lowercase();
    !DEVICE_NAMES.contains(&stem.as_str())
}

/// Whether two names are one file on Windows: compared the way NTFS compares names, by their
/// upper case, so `.RBROWSER` and `.rbrowſer` (`ſ` folds to `S`) are `.rbrowser`.
#[must_use]
pub fn same_name(name: &str, other: &str) -> bool {
    name.to_uppercase() == other.to_uppercase()
}

/// Whether `segment` names the config folder `.rbrowser`, in any case.
#[must_use]
pub fn is_config_dir(segment: &str) -> bool {
    same_name(segment, CONFIG_DIR)
}

/// Whether the project-relative `segments` name the machine-local config, which
/// `project-read` never hands out - in any case (review finding M3 of Charge 6).
#[must_use]
pub fn is_local_config(segments: &[&str]) -> bool {
    matches!(segments, [dir, file] if is_config_dir(dir) && same_name(file, LOCAL_CONFIG_FILE))
}

/// Whether the project-relative `segments` lie inside `.rbrowser/`, in any case.
#[must_use]
pub fn in_config_dir(segments: &[&str]) -> bool {
    segments.len() >= 2 && is_config_dir(segments[0])
}

/// Whether the project-relative `segments` lie in a folder `project-write` never writes into,
/// whatever its manifest's globs say: `.git/` (hooks and config run code) and `.rbrowser/` (the
/// project config, the trust-protected folders), at any depth and in any case.
#[must_use]
pub fn is_write_protected(segments: &[&str]) -> bool {
    // The last segment too: a worktree's or a submodule's `.git` is a file naming the real one.
    segments
        .iter()
        .any(|segment| is_config_dir(segment) || same_name(segment, VCS_DIR))
}

#[cfg(test)]
#[path = "paths_tests.rs"]
mod tests;
