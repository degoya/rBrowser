//! The baselines of a project: `.rbrowser/baselines/<slug>/<breakpoint>.png` and `meta.json`
//! beside them ([`crate::meta`]), checked in with Git LFS ([`crate::lfs`]).
//!
//! Every path is walked one folder at a time from the project root and **no link is followed**
//! (as `rb-plugin-host`'s `context/files.rs` does for plugins): a checked-in
//! `.rbrowser/baselines` that links elsewhere must not turn a baseline update into a write
//! there. Slug and breakpoint are plain names ([`crate::slug::valid_name`]), so nothing leaves
//! the folder. A file is written beside its target and renamed over it, so a crash leaves the old
//! baseline or the new one, never half of one.

use std::fs::File;
use std::io::{ErrorKind, Read as _};
use std::path::{Path, PathBuf};

use crate::VisualError;
use crate::meta::{BaselineMeta, MetaFile};
use crate::slug::valid_name;

/// The baselines' folder below the project root.
pub const BASELINES_DIR: [&str; 2] = [".rbrowser", "baselines"];
/// The metadata file of a page's folder.
pub const META_FILE: &str = "meta.json";
/// The largest baseline PNG read.
pub const MAX_BASELINE_BYTES: u64 = 256 * 1024 * 1024;
/// The largest `meta.json` read.
const MAX_META_BYTES: u64 = 1024 * 1024;

/// One stored baseline.
#[derive(Clone, Debug, PartialEq)]
pub struct Baseline {
    /// The PNG.
    pub png: Vec<u8>,
    /// Its entry of `meta.json`; `None` when the file or the entry is missing or unreadable.
    pub meta: Option<BaselineMeta>,
    /// Where it lies.
    pub path: PathBuf,
}

/// The baselines below one project root.
#[derive(Clone, Debug)]
pub struct BaselineStore {
    root: PathBuf,
}

fn refused(what: &str) -> VisualError {
    VisualError::Outside(what.to_owned())
}

fn io(path: &Path, source: std::io::Error) -> VisualError {
    VisualError::Io {
        path: path.display().to_string(),
        source,
    }
}

/// What is at `path`, without following a link; `None` when nothing is.
pub(crate) fn kind(path: &Path) -> Result<Option<std::fs::FileType>, VisualError> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) => Ok(Some(metadata.file_type())),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(io(path, error)),
    }
}

impl BaselineStore {
    /// The store of the project at `root`.
    pub fn new(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
        }
    }

    /// Where the baseline of `breakpoint` on the page `slug` lies (or would).
    pub fn png_path(&self, slug: &str, breakpoint: &str) -> PathBuf {
        self.folder_path(slug).join(format!("{breakpoint}.png"))
    }

    fn folder_path(&self, slug: &str) -> PathBuf {
        BASELINES_DIR
            .iter()
            .fold(self.root.clone(), |path, part| path.join(part))
            .join(slug)
    }

    /// The page's folder, each part an existing real folder (created with `create`); `None`
    /// when a part is missing and not created.
    fn walk(&self, slug: &str, create: bool) -> Result<Option<PathBuf>, VisualError> {
        if !valid_name(slug) {
            return Err(refused(slug));
        }
        let mut current = self.root.clone();
        for part in BASELINES_DIR.iter().copied().chain([slug]) {
            current.push(part);
            match kind(&current)? {
                Some(found) if found.is_symlink() => return Err(refused(part)),
                Some(found) if found.is_dir() => {}
                Some(_) => return Err(refused(part)),
                None if create => match std::fs::create_dir(&current) {
                    Ok(()) => {}
                    Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                        let found = kind(&current)?;
                        if !found.is_some_and(|found| found.is_dir() && !found.is_symlink()) {
                            return Err(refused(part));
                        }
                    }
                    Err(error) => return Err(io(&current, error)),
                },
                None => return Ok(None),
            }
        }
        Ok(Some(current))
    }

    /// The baseline of `breakpoint` on the page `slug`; `None` when there is none yet.
    pub fn read(&self, slug: &str, breakpoint: &str) -> Result<Option<Baseline>, VisualError> {
        if !valid_name(breakpoint) {
            return Err(refused(breakpoint));
        }
        let Some(folder) = self.walk(slug, false)? else {
            return Ok(None);
        };
        let path = folder.join(format!("{breakpoint}.png"));
        let Some(png) = read_file(&path, MAX_BASELINE_BYTES)? else {
            return Ok(None);
        };
        let meta = read_meta(&folder)?.baselines.remove(breakpoint);
        Ok(Some(Baseline { png, meta, path }))
    }

    /// Stores `png` as the baseline of `breakpoint` on the page `slug`, replacing the one
    /// before, and its `meta` in the page's `meta.json`; the PNG's path.
    pub fn write(
        &self,
        slug: &str,
        breakpoint: &str,
        png: &[u8],
        meta: &BaselineMeta,
    ) -> Result<PathBuf, VisualError> {
        if !valid_name(breakpoint) {
            return Err(refused(breakpoint));
        }
        let folder = self.walk(slug, true)?.ok_or_else(|| refused(slug))?;
        let path = folder.join(format!("{breakpoint}.png"));
        replace(&path, png)?;
        let mut file = read_meta(&folder)?;
        file.baselines.insert(breakpoint.to_owned(), meta.clone());
        let mut text = serde_json::to_string_pretty(&file)
            .map_err(|error| VisualError::Png(error.to_string()))?;
        text.push('\n');
        replace(&folder.join(META_FILE), text.as_bytes())?;
        Ok(path)
    }
}

/// The file at `path` when it is a regular file (never a link) of at most `limit` bytes;
/// `None` when nothing is there.
pub(crate) fn read_file(path: &Path, limit: u64) -> Result<Option<Vec<u8>>, VisualError> {
    match kind(path)? {
        None => return Ok(None),
        Some(found) if found.is_file() => {}
        Some(_) => return Err(refused(&path.display().to_string())),
    }
    let file = File::open(path).map_err(|error| io(path, error))?;
    let mut bytes = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| io(path, error))?;
    if bytes.len() as u64 > limit {
        return Err(VisualError::Png(format!(
            "{} is larger than {limit} bytes",
            path.display()
        )));
    }
    Ok(Some(bytes))
}

/// The page's `meta.json`; an empty one when it is missing or unreadable (a broken file is
/// replaced on the next write, and a baseline without metadata compares with a warning).
fn read_meta(folder: &Path) -> Result<MetaFile, VisualError> {
    let Some(bytes) = read_file(&folder.join(META_FILE), MAX_META_BYTES)? else {
        return Ok(MetaFile::default());
    };
    Ok(serde_json::from_slice(&bytes).unwrap_or_default())
}

/// Writes `bytes` to `path` through a new file beside it, renamed over it. A link or a folder
/// at `path` is refused, never replaced.
pub(crate) fn replace(path: &Path, bytes: &[u8]) -> Result<(), VisualError> {
    match kind(path)? {
        Some(found) if !found.is_file() => {
            return Err(refused(&path.display().to_string()));
        }
        _ => {}
    }
    if path.file_name().is_none() {
        return Err(refused(&path.display().to_string()));
    }
    rb_util::fs::write_atomic(path, bytes).map_err(|error| io(path, error))
}

#[cfg(test)]
#[path = "baseline_tests.rs"]
mod tests;
