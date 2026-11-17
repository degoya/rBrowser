//! The folder of one sitemap capture: `.rbrowser/out/sitemap-capture/<run>/` with `run.json`
//! ([`crate::sitemap`]), `<slug>/<breakpoint>.png` per shot and the gallery `index.html`.
//!
//! As for the baselines, every path is walked one folder at a time from the project root and
//! **no link is followed**; the run id, slugs and breakpoint names are plain names
//! ([`crate::slug::valid_name`]), so nothing is written outside the run's folder. Files are
//! written beside their target and renamed over it, so `run.json` is never half written and a
//! shot taken again replaces the one before (no `-2` copies on a resume).

use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::VisualError;
use crate::baseline::{kind, read_file, replace};
use crate::naming::Stamp;
use crate::sitemap::SitemapRun;
use crate::slug::valid_name;

/// The runs' folder below the project root.
pub const OUT_DIR: [&str; 3] = [".rbrowser", "out", "sitemap-capture"];
/// The state of a run.
pub const RUN_FILE: &str = "run.json";
/// The gallery of a run.
pub const GALLERY_FILE: &str = "index.html";
/// The largest `run.json` read.
pub const MAX_RUN_BYTES: u64 = 64 * 1024 * 1024;

fn refused(what: &str) -> VisualError {
    VisualError::Outside(what.to_owned())
}

/// The id of a run started at `stamp`: `2026-11-11_14-03-07`.
#[must_use]
pub fn run_id(stamp: &Stamp) -> String {
    stamp.date_time()
}

/// Walks `parts` below `start`, each an existing real folder (created with `create`); `None`
/// when one is missing and not created.
fn walk(start: &Path, parts: &[&str], create: bool) -> Result<Option<PathBuf>, VisualError> {
    let mut current = start.to_path_buf();
    for part in parts {
        current.push(part);
        match kind(&current)? {
            Some(found) if found.is_dir() && !found.is_symlink() => {}
            Some(_) => return Err(refused(part)),
            None if create => match std::fs::create_dir(&current) {
                Ok(()) => {}
                Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                    if !kind(&current)?.is_some_and(|found| found.is_dir() && !found.is_symlink()) {
                        return Err(refused(part));
                    }
                }
                Err(source) => {
                    return Err(VisualError::Io {
                        path: current.display().to_string(),
                        source,
                    });
                }
            },
            None => return Ok(None),
        }
    }
    Ok(Some(current))
}

/// The ids of the runs below `root`, oldest first ([`oldest_first`]).
///
/// # Errors
///
/// A link on the way, or the folder cannot be listed.
pub fn run_ids(root: &Path) -> Result<Vec<String>, VisualError> {
    run_ids_in(root, &OUT_DIR)
}

/// [`run_ids`] below the output folder `out_dir` (`[".rbrowser", "out", "env-compare"]`), and the
/// video recordings' folders.
///
/// # Errors
///
/// A link on the way, or the folder cannot be listed.
pub fn run_ids_in(root: &Path, out_dir: &[&str]) -> Result<Vec<String>, VisualError> {
    let Some(dir) = walk(root, out_dir, false)? else {
        return Ok(Vec::new());
    };
    let entries = std::fs::read_dir(&dir).map_err(|source| VisualError::Io {
        path: dir.display().to_string(),
        source,
    })?;
    let runs: Vec<(Option<SystemTime>, String)> = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            let made = entry
                .metadata()
                .ok()
                .and_then(|meta| meta.created().or_else(|_| meta.modified()).ok());
            Some((made, name))
        })
        .filter(|(_, name)| valid_name(name))
        .collect();
    Ok(oldest_first(runs))
}

/// The run ids of `runs` (when each folder was made, its name), oldest first: by that time - a
/// name in local time does not sort by time across the fall back of daylight saving time, nor
/// after names in UTC (RB-090-69) - and by name at the same time or without one (first).
pub fn oldest_first(mut runs: Vec<(Option<SystemTime>, String)>) -> Vec<String> {
    runs.sort();
    runs.into_iter().map(|(_, name)| name).collect()
}

/// One run's folder.
#[derive(Clone, Debug)]
pub struct RunStore {
    dir: PathBuf,
}

impl RunStore {
    /// The folder of run `run_id` below `root`, created with `create`; `None` when it does not
    /// exist and is not created.
    ///
    /// # Errors
    ///
    /// The id is no plain name, a link or file is on the way, or a folder cannot be made.
    pub fn open(root: &Path, run_id: &str, create: bool) -> Result<Option<Self>, VisualError> {
        Self::open_in(root, &OUT_DIR, run_id, create)
    }

    /// [`RunStore::open`] below the output folder `out_dir` instead of the sitemap capture's
    /// (the environment comparison, RB-080-03).
    ///
    /// # Errors
    ///
    /// The id is no plain name, a link or file is on the way, or a folder cannot be made.
    pub fn open_in(
        root: &Path,
        out_dir: &[&str],
        run_id: &str,
        create: bool,
    ) -> Result<Option<Self>, VisualError> {
        if !valid_name(run_id) {
            return Err(refused(run_id));
        }
        let mut parts = out_dir.to_vec();
        parts.push(run_id);
        Ok(walk(root, &parts, create)?.map(|dir| Self { dir }))
    }

    /// The run's folder.
    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Writes `run.json`.
    ///
    /// # Errors
    ///
    /// The file cannot be written, or a link or folder stands where it goes.
    pub fn save(&self, run: &SitemapRun) -> Result<(), VisualError> {
        let mut text = serde_json::to_string_pretty(run)
            .map_err(|error| VisualError::Png(error.to_string()))?;
        text.push('\n');
        replace(&self.dir.join(RUN_FILE), text.as_bytes())
    }

    /// `run.json`; `None` when there is none.
    ///
    /// # Errors
    ///
    /// It is a link, larger than [`MAX_RUN_BYTES`] or no run.
    pub fn load(&self) -> Result<Option<SitemapRun>, VisualError> {
        let Some(bytes) = read_file(&self.dir.join(RUN_FILE), MAX_RUN_BYTES)? else {
            return Ok(None);
        };
        serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|error| VisualError::Png(format!("{RUN_FILE}: {error}")))
    }

    /// Writes the shot of `breakpoint` on the page `slug`; its path relative to the run's folder
    /// (`about/mobile.png`).
    ///
    /// # Errors
    ///
    /// A name that is not plain, a link on the way, or the file cannot be written.
    pub fn write_shot(
        &self,
        slug: &str,
        breakpoint: &str,
        png: &[u8],
    ) -> Result<String, VisualError> {
        if !valid_name(breakpoint) {
            return Err(refused(breakpoint));
        }
        if !valid_name(slug) {
            return Err(refused(slug));
        }
        let folder = walk(&self.dir, &[slug], true)?.ok_or_else(|| refused(slug))?;
        let name = format!("{breakpoint}.png");
        replace(&folder.join(&name), png)?;
        Ok(format!("{slug}/{name}"))
    }

    /// The file `name` of the run's folder, at most `limit` bytes; `None` when it is not there.
    ///
    /// # Errors
    ///
    /// A name that is not plain, a link, a file larger than `limit`, or a read that fails.
    pub fn read(&self, name: &str, limit: u64) -> Result<Option<Vec<u8>>, VisualError> {
        if !valid_name(name) {
            return Err(refused(name));
        }
        read_file(&self.dir.join(name), limit)
    }

    /// Writes `bytes` as the file `name` of the run's folder (the gallery); its path.
    ///
    /// # Errors
    ///
    /// A name that is not plain, or the file cannot be written.
    pub fn write_file(&self, name: &str, bytes: &[u8]) -> Result<PathBuf, VisualError> {
        if !valid_name(name) {
            return Err(refused(name));
        }
        let path = self.dir.join(name);
        replace(&path, bytes)?;
        Ok(path)
    }
}

#[cfg(test)]
#[path = "sitemap_store_tests.rs"]
mod tests;
