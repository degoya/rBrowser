//! The recorder's exports (RB-050-02): the request the host sends the `recorder-export` plugin
//! ([`ExportRequest`] - the recording, the locator preference, the base URL and one Playwright
//! project per breakpoint) and the folder the answers are written to ([`ExportsDir`],
//! `automation.recorderExport.dir`, default `.rbrowser/out/exports`).
//!
//! The folder follows the recordings folder's rules ([`RecordingStore::open`]): inside the
//! project's `.rbrowser/`, outside the folders workspace trust protects, also after links are
//! followed. Every export is a new folder `<name>/` (`<name>-2/`, ...) created atomically, so
//! nothing is ever overwritten, and a spec and its `playwright.config.ts` stay together.

use std::fs;
use std::io::{ErrorKind, Write as _};
use std::path::{Path, PathBuf};

use serde::Serialize;

use super::model::Recording;
use super::store::{MAX_NAME, RecordingStore, StoreError, check_name};

/// The default of `automation.recorderExport.dir`.
pub const DEFAULT_EXPORTS_DIR: &str = ".rbrowser/out/exports";
/// How many suffixes [`ExportsDir::write_new`] tries.
const MAX_SUFFIX: u32 = 999;
/// The longest file name an export may have.
const MAX_FILE_NAME: usize = 96;

/// What the host sends the exporter (`plugins/recorder-export/src/model.rs` reads it).
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportRequest<'a> {
    /// The recording.
    pub recording: &'a Recording,
    /// How to export it.
    pub options: ExportOptions,
}

/// How to export.
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportOptions {
    /// `auto`, `testid`, `role`, `text` or `css`.
    pub locator_preference: String,
    /// The origin URLs in the spec are relative to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    /// One Playwright project per breakpoint.
    pub projects: Vec<ExportProject>,
}

/// A breakpoint as a Playwright project.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportProject {
    /// The breakpoint id.
    pub name: String,
    /// `chromium`, `firefox` or `webkit`.
    pub engine: String,
    /// Viewport width in CSS px.
    pub width: u32,
    /// Viewport height in CSS px.
    pub height: u32,
    /// Device pixel ratio; absent keeps Playwright's.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_scale_factor: Option<f64>,
    /// Mobile device type.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_mobile: Option<bool>,
    /// Touch input.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub has_touch: Option<bool>,
    /// User agent override.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_agent: Option<String>,
    /// BCP 47 locale.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub locale: Option<String>,
    /// `light` or `dark`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color_scheme: Option<String>,
    /// `prefers-reduced-motion: reduce`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reduced_motion: Option<bool>,
}

/// Why an export was not written.
#[derive(Debug, thiserror::Error)]
pub enum ExportsError {
    /// `automation.recorderExport.dir` is not a folder inside `.rbrowser/` outside the
    /// protected ones.
    #[error("the exports folder {0:?} must lie inside .rbrowser/, outside its protected folders")]
    BadDir(String),
    /// The export's name is no recording name.
    #[error("invalid export name {0:?}")]
    BadName(String),
    /// A file name the exporter answered has a folder or characters a file may not have.
    #[error("invalid export file name {0:?}")]
    BadFileName(String),
    /// Every suffix is taken.
    #[error("no free export folder for {0:?}")]
    Exists(String),
    /// Writing failed.
    #[error("{0}")]
    Io(#[from] std::io::Error),
}

impl From<StoreError> for ExportsError {
    fn from(error: StoreError) -> Self {
        match error {
            StoreError::Io(error) => Self::Io(error),
            StoreError::BadName(name) => Self::BadName(name),
            StoreError::BadDir(dir) => Self::BadDir(dir),
            other => Self::BadDir(other.to_string()),
        }
    }
}

/// An export that was written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Exported {
    /// Its folder.
    pub folder: PathBuf,
    /// Its files, in the order given.
    pub files: Vec<PathBuf>,
}

/// The exports folder of one project.
#[derive(Clone, Debug)]
pub struct ExportsDir {
    store: RecordingStore,
}

impl ExportsDir {
    /// The exports folder `dir` (relative, `/` or `\`) of the project at `root`, never inside
    /// one of `protected` (folder names below `.rbrowser/`).
    pub fn open(root: &Path, dir: &str, protected: &[&str]) -> Result<Self, ExportsError> {
        let store = RecordingStore::open(root, dir, protected).map_err(|error| match error {
            StoreError::BadDir(_) => ExportsError::BadDir(dir.to_owned()),
            other => other.into(),
        })?;
        Ok(Self { store })
    }

    /// The folder.
    pub fn dir(&self) -> &Path {
        self.store.dir()
    }

    /// Writes `files` (name, text) into a new folder `<stem>/`, or the next free
    /// `<stem>-<n>/`; nothing is overwritten.
    pub fn write_new(
        &self,
        stem: &str,
        files: &[(String, String)],
    ) -> Result<Exported, ExportsError> {
        check_name(stem)?;
        if let Some((name, _)) = files.iter().find(|(name, _)| !valid_file_name(name)) {
            return Err(ExportsError::BadFileName(name.clone()));
        }
        self.store.prepare()?;
        let folder = self.new_folder(stem)?;
        self.store.prepare()?;
        let mut written = Vec::with_capacity(files.len());
        for (name, text) in files {
            let path = folder.join(name);
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)?;
            file.write_all(text.as_bytes())?;
            file.sync_all()?;
            written.push(path);
        }
        Ok(Exported {
            folder,
            files: written,
        })
    }

    /// Creates `<stem>/` or the next free `<stem>-<n>/`.
    fn new_folder(&self, stem: &str) -> Result<PathBuf, ExportsError> {
        for n in 1..=MAX_SUFFIX {
            let name = if n == 1 {
                stem.to_owned()
            } else {
                let suffix = format!("-{n}");
                let keep = MAX_NAME.saturating_sub(suffix.len()).min(stem.len());
                format!("{}{suffix}", &stem[..keep])
            };
            let folder = self.store.dir().join(&name);
            match fs::create_dir(&folder) {
                Ok(()) => return Ok(folder),
                Err(error) if error.kind() == ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error.into()),
            }
        }
        Err(ExportsError::Exists(stem.to_owned()))
    }
}

/// A file name of one segment: `[A-Za-z0-9._-]`, not starting with a dot, at most
/// [`MAX_FILE_NAME`] characters, its stem no Windows device name.
fn valid_file_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_FILE_NAME
        && !name.starts_with('.')
        && !super::store::is_device_name(name.split('.').next().unwrap_or(name))
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

#[cfg(test)]
#[path = "exports_tests.rs"]
mod tests;
