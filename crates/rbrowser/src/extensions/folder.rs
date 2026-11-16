//! Unpacked extension folders: one the person names (an absolute path, or the `manifest.json`
//! in it) and one a trusted project recommends (a path inside the project, never out of it, a
//! link included). A folder is referenced where it is, never copied.

use std::io::Read as _;
use std::path::{Component, Path, PathBuf};

use super::manifest::{MANIFEST_FILE, MAX_MANIFEST_BYTES, Manifest, ManifestError, parse};

/// A checked extension folder.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unpacked {
    /// The folder holding `manifest.json`, as named.
    pub folder: PathBuf,
    /// Its manifest.
    pub manifest: Manifest,
}

/// Why a folder is no extension rBrowser loads.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum FolderError {
    /// No folder there.
    #[error("there is no folder at this path")]
    Missing,
    /// A relative path: it would depend on the working folder.
    #[error("the path is not absolute")]
    NotAbsolute,
    /// Chromium's `--load-extension` separates folders with commas.
    #[error("the path contains a comma, which Chromium cannot load an extension from")]
    Comma,
    /// No `manifest.json` in it.
    #[error("the folder has no manifest.json")]
    NoManifest,
    /// The manifest is larger than any real one.
    #[error("manifest.json is larger than 1 MiB")]
    TooLarge,
    /// The manifest cannot be read.
    #[error("manifest.json cannot be read: {0}")]
    Unreadable(String),
    /// The manifest is refused.
    #[error(transparent)]
    Manifest(#[from] ManifestError),
    /// A project's path that leaves the project.
    #[error("the path is not a folder inside the project")]
    OutsideProject,
}

impl FolderError {
    /// The stable code of the error (MCP answers, notices).
    pub fn code(&self) -> &'static str {
        match self {
            Self::Missing => "extensions.folder_missing",
            Self::NotAbsolute => "extensions.path_not_absolute",
            Self::Comma => "extensions.path_has_comma",
            Self::NoManifest => "extensions.no_manifest",
            Self::TooLarge => "extensions.manifest_too_large",
            Self::Unreadable(_) => "extensions.manifest_unreadable",
            Self::Manifest(error) => error.code(),
            Self::OutsideProject => "extensions.path_outside_project",
        }
    }
}

/// Checks the folder at `path` (or the folder of the `manifest.json` at `path`).
///
/// # Errors
///
/// Why it is no extension rBrowser loads.
pub fn check(path: &str) -> Result<Unpacked, FolderError> {
    let path = path.trim();
    if path.is_empty() {
        return Err(FolderError::Missing);
    }
    if path.contains(',') {
        return Err(FolderError::Comma);
    }
    let mut folder = PathBuf::from(path);
    if !folder.is_absolute() {
        return Err(FolderError::NotAbsolute);
    }
    let names_manifest = folder
        .file_name()
        .is_some_and(|name| name.eq_ignore_ascii_case(MANIFEST_FILE));
    if names_manifest && folder.is_file() {
        folder.pop();
    }
    if !folder.is_dir() {
        return Err(FolderError::Missing);
    }
    let manifest = read_manifest(&folder.join(MANIFEST_FILE))?;
    Ok(Unpacked { folder, manifest })
}

fn read_manifest(path: &Path) -> Result<Manifest, FolderError> {
    let file = std::fs::File::open(path).map_err(|error| match error.kind() {
        std::io::ErrorKind::NotFound => FolderError::NoManifest,
        _ => FolderError::Unreadable(error.to_string()),
    })?;
    let mut text = String::new();
    file.take(MAX_MANIFEST_BYTES + 1)
        .read_to_string(&mut text)
        .map_err(|error| FolderError::Unreadable(error.to_string()))?;
    if u64::try_from(text.len()).unwrap_or(u64::MAX) > MAX_MANIFEST_BYTES {
        return Err(FolderError::TooLarge);
    }
    Ok(parse(text.trim_start_matches('\u{feff}'))?)
}

/// The folder a project's recommendation `written` names, relative to `root`: no absolute
/// path, no drive, no `..`, and - when it exists - no link that leads out of the project.
///
/// # Errors
///
/// [`FolderError::OutsideProject`] for every path that is not plainly inside.
pub fn project_folder(root: &Path, written: &str) -> Result<PathBuf, FolderError> {
    let written = written.trim();
    let plain = !written.is_empty()
        && !written.starts_with(['/', '\\'])
        && !written.contains(':')
        && written.split(['/', '\\']).all(|segment| segment != "..");
    if !plain {
        return Err(FolderError::OutsideProject);
    }
    let mut folder = root.to_path_buf();
    for segment in written
        .split(['/', '\\'])
        .filter(|segment| !segment.is_empty() && *segment != ".")
    {
        folder.push(segment);
    }
    // Only normal parts were added; a link inside may still lead out.
    let relative_ok = folder
        .strip_prefix(root)
        .is_ok_and(|rest| rest.components().all(|c| matches!(c, Component::Normal(_))));
    if !relative_ok {
        return Err(FolderError::OutsideProject);
    }
    if folder.exists() {
        let real_root = root
            .canonicalize()
            .map_err(|_| FolderError::OutsideProject)?;
        let real = folder
            .canonicalize()
            .map_err(|_| FolderError::OutsideProject)?;
        if !real.starts_with(&real_root) {
            return Err(FolderError::OutsideProject);
        }
    }
    Ok(folder)
}

#[cfg(test)]
#[path = "folder_tests.rs"]
mod tests;
