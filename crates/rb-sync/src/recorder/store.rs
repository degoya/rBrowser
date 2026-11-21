//! Where recordings live (RB-050-01): `<dir>/<name>.json`, `<dir>` being
//! `automation.recorder.dir` relative to the project root (default `.rbrowser/out/recordings`).
//!
//! The folder must lie inside the project's `.rbrowser/` and outside its protected folders
//! (`flows`, `snippets`, `plugins`, `mocks`, `scripts` - workspace trust hashes those, so a
//! recording written there would take the trust away), also after links are followed. A file
//! is never overwritten: [`RecordingStore::save`] refuses an existing name, and
//! [`RecordingStore::save_new`] picks the next free one (`name-2`, `name-3`, ...); only the
//! Recorder panel's save writes over its own file ([`RecordingStore::replace`]) and its "Delete"
//! removes one ([`RecordingStore::delete`]). Every read-modify-write of a recording holds
//! [`write_lock`], and each replace writes through a temporary file of its own (RB-092-01).

use std::fs;
use std::io::{ErrorKind, Write as _};
use std::path::{Component, Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use super::export::{ExportError, from_json, to_json};
use super::model::Recording;

/// The default of `automation.recorder.dir`.
pub const DEFAULT_DIR: &str = ".rbrowser/out/recordings";
/// The project's own folder.
const CONFIG_DIR: &str = ".rbrowser";
/// The extension of a recording.
pub const EXTENSION: &str = "json";
/// The longest name.
pub const MAX_NAME: usize = 64;
/// How many suffixes [`RecordingStore::save_new`] tries.
const MAX_SUFFIX: u32 = 999;

/// The lock every read-modify-write of a stored recording holds (RB-092-01): the MCP step
/// tools, the Recorder panel's save and a continued recording's save, so none loses another's
/// update. Process-wide: one app writes a project's recordings.
static WRITE_LOCK: Mutex<()> = Mutex::new(());

/// Takes the lock of every read-modify-write of a stored recording; held until dropped.
pub fn write_lock() -> MutexGuard<'static, ()> {
    rb_util::lock(&WRITE_LOCK)
}

/// Why a recording could not be stored or read.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    /// `automation.recorder.dir` is not a folder inside `.rbrowser/` outside the protected
    /// ones.
    #[error(
        "the recordings folder {0:?} must lie inside .rbrowser/, outside its protected folders"
    )]
    BadDir(String),
    /// The name is empty, too long or has other characters than `[A-Za-z0-9_-]`.
    #[error("invalid recording name {0:?}")]
    BadName(String),
    /// A recording of that name exists; it is not overwritten.
    #[error("a recording named {0:?} exists already")]
    Exists(String),
    /// No recording of that name.
    #[error("no recording named {0:?}")]
    NotFound(String),
    /// Reading or writing failed.
    #[error("{0}")]
    Io(#[from] std::io::Error),
    /// The file is not a recording.
    #[error(transparent)]
    Format(#[from] ExportError),
}

/// The recordings folder of one project.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordingStore {
    config: PathBuf,
    dir: PathBuf,
    protected: Vec<String>,
}

impl RecordingStore {
    /// The store of the project at `root`, folder `dir` (relative, `/` or `\`), never inside one
    /// of `protected` (folder names below `.rbrowser/`).
    pub fn open(root: &Path, dir: &str, protected: &[&str]) -> Result<Self, StoreError> {
        let bad = || StoreError::BadDir(dir.to_owned());
        let relative = PathBuf::from(dir.replace('\\', "/"));
        let parts: Vec<&str> = relative
            .components()
            .map(|part| match part {
                Component::Normal(name) => name.to_str().ok_or_else(bad),
                Component::CurDir => Ok("."),
                _ => Err(bad()),
            })
            .filter(|part| !matches!(part, Ok(".")))
            .collect::<Result<_, _>>()?;
        match parts.as_slice() {
            [first, second, ..]
                if first.eq_ignore_ascii_case(CONFIG_DIR)
                    && !protected
                        .iter()
                        .any(|name| name.eq_ignore_ascii_case(second)) => {}
            _ => return Err(bad()),
        }
        let dir = parts
            .iter()
            .fold(root.to_path_buf(), |path, part| path.join(part));
        Ok(Self {
            config: root.join(parts[0]),
            dir,
            protected: protected.iter().map(|name| (*name).to_owned()).collect(),
        })
    }

    /// The folder.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The file of the recording `name`.
    pub fn path_of(&self, name: &str) -> Result<PathBuf, StoreError> {
        check_name(name)?;
        Ok(self.dir.join(format!("{name}.{EXTENSION}")))
    }

    /// Writes `recording` as `<name>.json`; refuses when that file exists.
    pub fn save(&self, recording: &Recording) -> Result<PathBuf, StoreError> {
        let path = self.path_of(&recording.name)?;
        self.prepare()?;
        let text = to_json(recording)?;
        let mut file = match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(file) => file,
            Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                return Err(StoreError::Exists(recording.name.clone()));
            }
            Err(error) => return Err(error.into()),
        };
        file.write_all(text.as_bytes())?;
        file.sync_all()?;
        Ok(path)
    }

    /// Writes `recording` under its name, or the next free `name-<n>` (the name is updated).
    pub fn save_new(&self, recording: &mut Recording) -> Result<PathBuf, StoreError> {
        let base = recording.name.clone();
        check_name(&base)?;
        for n in 1..=MAX_SUFFIX {
            if n > 1 {
                let suffix = format!("-{n}");
                let keep = MAX_NAME.saturating_sub(suffix.len()).min(base.len());
                recording.name = format!("{}{suffix}", &base[..keep]);
            }
            match self.save(recording) {
                Err(StoreError::Exists(_)) => continue,
                other => return other,
            }
        }
        Err(StoreError::Exists(base))
    }

    /// Writes `recording` over its existing file (the Recorder panel's save, RB-090-60): into a
    /// temporary file beside it first, then renamed over it, so a failed write leaves the old
    /// file whole. Refuses a name without a file.
    pub fn replace(&self, recording: &Recording) -> Result<PathBuf, StoreError> {
        let path = self.path_of(&recording.name)?;
        self.inside()?;
        if !path.is_file() {
            return Err(StoreError::NotFound(recording.name.clone()));
        }
        let text = to_json(recording)?;
        // A temporary file of its own per write, so two writers never share one (RB-092-01).
        rb_util::fs::write_atomic(&path, text.as_bytes())?;
        Ok(path)
    }

    /// Reads the recording `name`.
    pub fn load(&self, name: &str) -> Result<Recording, StoreError> {
        let path = self.path_of(name)?;
        self.inside()?;
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) if error.kind() == ErrorKind::NotFound => {
                return Err(StoreError::NotFound(name.to_owned()));
            }
            Err(error) => return Err(error.into()),
        };
        Ok(from_json(&text)?)
    }

    /// Deletes the recording `name` (the Recorder panel's "Delete", RB-090-91); refuses a name
    /// without a file.
    pub fn delete(&self, name: &str) -> Result<(), StoreError> {
        let path = self.path_of(name)?;
        self.inside()?;
        // A link goes alone; what it points to stays.
        if !path.is_file() {
            return Err(StoreError::NotFound(name.to_owned()));
        }
        fs::remove_file(&path)?;
        Ok(())
    }

    /// The names of the stored recordings, sorted.
    pub fn list(&self) -> Result<Vec<String>, StoreError> {
        let entries = match fs::read_dir(&self.dir) {
            Ok(entries) => entries,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(error.into()),
        };
        self.inside()?;
        let mut names: Vec<String> = entries
            .filter_map(Result::ok)
            .filter_map(|entry| {
                let path = entry.path();
                let stem = path.file_stem()?.to_str()?.to_owned();
                (path.extension()? == EXTENSION && check_name(&stem).is_ok()).then_some(stem)
            })
            .collect();
        names.sort();
        Ok(names)
    }

    /// Checks the folder, then creates it and checks again.
    pub(crate) fn prepare(&self) -> Result<(), StoreError> {
        self.inside()?;
        fs::create_dir_all(&self.dir)?;
        self.inside()
    }

    /// The folder - or, before it exists, its deepest existing ancestor - lies below the
    /// project's `.rbrowser/` and outside the protected folders once links are followed.
    fn inside(&self) -> Result<(), StoreError> {
        let bad = || StoreError::BadDir(self.dir.display().to_string());
        // `.rbrowser/` itself a link would carry the whole check out of the project.
        if fs::symlink_metadata(&self.config).is_ok_and(|meta| meta.file_type().is_symlink()) {
            return Err(bad());
        }
        let config = fs::canonicalize(&self.config)?;
        let existing = self
            .dir
            .ancestors()
            .find(|path| path.exists())
            .ok_or_else(bad)?;
        let whole = existing == self.dir;
        let existing = fs::canonicalize(existing)?;
        let below = existing.strip_prefix(&config).map_err(|_| bad())?;
        match below
            .components()
            .next()
            .map(|part| part.as_os_str().to_str())
        {
            Some(Some(first))
                if !self
                    .protected
                    .iter()
                    .any(|name| name.eq_ignore_ascii_case(first)) =>
            {
                Ok(())
            }
            None if !whole => Ok(()),
            _ => Err(bad()),
        }
    }
}

/// A name is `[A-Za-z0-9_-]`, 1 to [`MAX_NAME`] characters, and no Windows device name.
pub fn check_name(name: &str) -> Result<(), StoreError> {
    let ok = !name.is_empty()
        && name.len() <= MAX_NAME
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        && !is_device_name(name);
    if ok {
        Ok(())
    } else {
        Err(StoreError::BadName(name.to_owned()))
    }
}

/// Whether `stem` (a file name up to its first dot) is a Windows device - `CON`, `PRN`, `AUX`,
/// `NUL`, `COM1` to `COM9`, `LPT1` to `LPT9`, any case: Windows opens the device, not a file.
pub(crate) fn is_device_name(stem: &str) -> bool {
    let upper = stem.to_ascii_uppercase();
    match upper.as_bytes() {
        [b'C', b'O', b'N'] | [b'P', b'R', b'N'] | [b'A', b'U', b'X'] | [b'N', b'U', b'L'] => true,
        [b'C', b'O', b'M', digit] | [b'L', b'P', b'T', digit] => (b'1'..=b'9').contains(digit),
        _ => false,
    }
}

#[cfg(test)]
#[path = "store_tests.rs"]
mod tests;
