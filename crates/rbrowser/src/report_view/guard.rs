//! Which files the report window may show and which run folders may go (RB-091-02): only below
//! `.rbrowser/out/<plugin-id>/` of the open project, only in a trusted workspace. The path is
//! checked as text first ([`rb_plugin_types::paths::relative_segments`], the rule of every plugin
//! path: no `..`, nothing absolute, no drive or stream colon, no device or short name), then
//! walked one segment at a time from the project root with no link followed - a link anywhere on
//! the way (`.rbrowser/out` itself included) refuses the path, as for the plugins' own file
//! access (`rb-plugin-host`, `context/files.rs`). A report is an existing `.html` file of at most
//! [`MAX_REPORT_BYTES`]; a run folder is a real folder directly below the plugin's own.
//!
//! The check runs when a report is asked for and again on every load of the window
//! (`cef_app/report_scheme.rs`), so a file swapped for a link in between is not served.

use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use rb_plugin_types::paths::relative_segments;

/// The largest report the window loads: 64 MiB (Lighthouse writes at most 8 MiB).
pub const MAX_REPORT_BYTES: u64 = 64 * 1024 * 1024;

/// The folders from the project root to every plugin's out folder.
const OUT: [&str; 2] = [".rbrowser", "out"];

/// Why a report or a run folder is refused. The code goes to the log, the key to the person;
/// neither carries a path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// No project is open.
    NoProject,
    /// The workspace is not trusted.
    Untrusted,
    /// The path is no relative path below the plugin's folder (`..`, absolute, a bad name).
    Path,
    /// A link on the way.
    Link,
    /// Nothing there, or not the kind asked for.
    NotFound,
    /// A report that is no `.html` file.
    NotHtml,
    /// A report over [`MAX_REPORT_BYTES`].
    TooLarge,
    /// The disk refused.
    Io,
}

impl Refusal {
    /// Every refusal.
    pub const ALL: [Self; 8] = [
        Self::NoProject,
        Self::Untrusted,
        Self::Path,
        Self::Link,
        Self::NotFound,
        Self::NotHtml,
        Self::TooLarge,
        Self::Io,
    ];

    /// The code for the log and an answer.
    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            Self::NoProject => "report_no_project",
            Self::Untrusted => "report_untrusted",
            Self::Path => "report_path",
            Self::Link => "report_link",
            Self::NotFound => "report_not_found",
            Self::NotHtml => "report_not_html",
            Self::TooLarge => "report_too_large",
            Self::Io => "report_io",
        }
    }

    /// The key of the sentence the status bar shows.
    #[must_use]
    pub fn key(self) -> &'static str {
        match self {
            Self::NoProject => "reports.error.no_project",
            Self::Untrusted => "reports.error.untrusted",
            Self::Path => "reports.error.path",
            Self::Link => "reports.error.link",
            Self::NotFound => "reports.error.not_found",
            Self::NotHtml => "reports.error.not_html",
            Self::TooLarge => "reports.error.too_large",
            Self::Io => "reports.error.io",
        }
    }
}

/// The project root a report is read from or a run deleted in: only an open project in a
/// trusted workspace.
///
/// # Errors
///
/// [`Refusal::NoProject`], [`Refusal::Untrusted`].
pub fn admit(root: Option<PathBuf>, trusted: bool) -> Result<PathBuf, Refusal> {
    let root = root.ok_or(Refusal::NoProject)?;
    if trusted {
        Ok(root)
    } else {
        Err(Refusal::Untrusted)
    }
}

/// The segments of `rel` below plugin `plugin`'s out folder, from the project root.
fn segments<'a>(plugin: &'a str, rel: &'a str) -> Result<Vec<&'a str>, Refusal> {
    if !rb_plugin_host::valid_plugin_id(plugin) {
        return Err(Refusal::Path);
    }
    let below = relative_segments(rel).map_err(|_| Refusal::Path)?;
    let mut all: Vec<&str> = OUT.to_vec();
    all.push(plugin);
    all.extend(below);
    Ok(all)
}

/// `root` joined with `folders`, each an existing real folder, none a link.
fn walk(root: &Path, folders: &[&str]) -> Result<PathBuf, Refusal> {
    let mut current = root.to_path_buf();
    for folder in folders {
        current.push(folder);
        let found = kind(&current)?.ok_or(Refusal::NotFound)?;
        if found.is_symlink() {
            return Err(Refusal::Link);
        }
        if !found.is_dir() {
            return Err(Refusal::NotFound);
        }
    }
    Ok(current)
}

/// What is at `path`, without following a link: `Ok(None)` when nothing is.
fn kind(path: &Path) -> Result<Option<std::fs::FileType>, Refusal> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) => Ok(Some(metadata.file_type())),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(_) => Err(Refusal::Io),
    }
}

/// The report `rel` (relative to `.rbrowser/out/<plugin>/`, such as `<run>/mobile.html`) of the
/// project at `root`.
///
/// # Errors
///
/// Every [`Refusal`] but the project's own; see the module documentation.
pub fn report_file(root: &Path, plugin: &str, rel: &str) -> Result<PathBuf, Refusal> {
    let all = segments(plugin, rel)?;
    let (file, folders) = all.split_last().ok_or(Refusal::Path)?;
    if !file.to_ascii_lowercase().ends_with(".html") {
        return Err(Refusal::NotHtml);
    }
    let path = walk(root, folders)?.join(file);
    let metadata = match std::fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == ErrorKind::NotFound => return Err(Refusal::NotFound),
        Err(_) => return Err(Refusal::Io),
    };
    if metadata.file_type().is_symlink() {
        return Err(Refusal::Link);
    }
    if !metadata.is_file() {
        return Err(Refusal::NotFound);
    }
    if metadata.len() > MAX_REPORT_BYTES {
        return Err(Refusal::TooLarge);
    }
    Ok(path)
}

/// The run folder `rel` - one name, directly below `.rbrowser/out/<plugin>/` - of the project at
/// `root`.
///
/// # Errors
///
/// Every [`Refusal`] but the project's own and the report's; a path of more than one name is
/// [`Refusal::Path`].
pub fn run_folder(root: &Path, plugin: &str, rel: &str) -> Result<PathBuf, Refusal> {
    let all = segments(plugin, rel)?;
    if all.len() != OUT.len() + 2 {
        return Err(Refusal::Path);
    }
    walk(root, &all)
}

/// The bytes of `path`, read no further than `limit` + 1: a file over `limit` - grown since it
/// was checked - is refused instead of read whole (security review L2).
///
/// # Errors
///
/// [`Refusal::TooLarge`] over `limit`, [`Refusal::Io`] when it cannot be read.
pub fn read_bounded(path: &Path, limit: u64) -> Result<Vec<u8>, Refusal> {
    use std::io::Read as _;
    let file = std::fs::File::open(path).map_err(|_| Refusal::Io)?;
    let mut bytes = Vec::new();
    file.take(limit.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|_| Refusal::Io)?;
    if bytes.len() as u64 > limit {
        return Err(Refusal::TooLarge);
    }
    Ok(bytes)
}

#[cfg(test)]
#[path = "guard_tests.rs"]
mod tests;
