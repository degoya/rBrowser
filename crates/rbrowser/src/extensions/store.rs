//! The Web Store extensions on disk (RB-080-23): `<root>/<id>/<version>/` with the unpacked
//! files and `<root>/<id>/current.json` naming the active version; the root is
//! `%LOCALAPPDATA%\rBrowser\extensions` ([`ExtensionStore::default_root`]). An active version
//! is loaded like an unpacked folder (`super::start`).
//!
//! An install takes a [`Verified`] CRX only - nothing reaches here unverified - and runs in
//! steps, each undone when a later one fails: unpack into `<root>/.staging/<id>-<pid>-<n>/`
//! (every member name checked: no absolute path, no `..`, no drive, no backslash, no two names
//! alike in letter case, as the file system folds it; at most [`Limits::DEFAULT`] files and
//! bytes), the `manifest.json` checked as an unpacked folder's
//! (Manifest V3, no `debugger`) with the developer's key written into it, so Chromium keeps the
//! Web Store id for the unpacked load; then the version folder renamed into place, the pointer
//! replaced atomically, the other versions removed. One install per extension folder at a time
//! ([`InProgress`]): a second one of the same id is refused while the first runs, and every
//! attempt stages under a name of its own, so no two ever touch one folder. The store is shared
//! by every rBrowser instance (one per project): a removal reads the staging names' pids
//! ([`alive`]) and is refused while another window's install of the id runs. Blocking: the
//! callers run it on a blocking thread.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use base64::Engine as _;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::crx3::Verified;
use super::folder::{FolderError, check};
use super::manifest::MANIFEST_FILE;
use super::webstore::is_id;
use crate::cdp::lock;

mod alive;
mod unpack;
pub use alive::process_alive;
use alive::staging_pid;
pub use unpack::peek_manifest;
use unpack::unpack;

/// The staging folder below the root.
pub const STAGING: &str = ".staging";
/// The pointer file in an extension's folder.
pub const POINTER: &str = "current.json";

/// The extension folders an install is under way for (process-wide: one store per process).
static IN_PROGRESS: Mutex<BTreeSet<PathBuf>> = Mutex::new(BTreeSet::new());
/// The install attempts so far, part of every staging name.
static ATTEMPTS: AtomicU64 = AtomicU64::new(0);

/// An extension folder taken for one install; free again when dropped.
pub struct InProgress(PathBuf);

impl InProgress {
    /// Takes `dir`; `None` while another install holds it.
    pub fn take(dir: PathBuf) -> Option<Self> {
        // The guard is gone before `Self` exists: its drop takes the same lock.
        let taken = lock(&IN_PROGRESS).insert(dir.clone());
        taken.then(|| Self(dir))
    }
}

impl Drop for InProgress {
    fn drop(&mut self) {
        lock(&IN_PROGRESS).remove(&self.0);
    }
}

/// A name no other attempt of this process uses: `<id>-<pid>-<n>`.
fn attempt_name(id: &str) -> String {
    format!(
        "{id}-{}-{}",
        std::process::id(),
        ATTEMPTS.fetch_add(1, Ordering::Relaxed)
    )
}

/// What one install may unpack.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    /// The most bytes, all members together.
    pub bytes: u64,
    /// The most members.
    pub files: usize,
}

impl Limits {
    /// A decompression bomb stops here; a real extension is a few MB and some hundred files.
    pub const DEFAULT: Limits = Limits {
        bytes: 256 * 1024 * 1024,
        files: 10_000,
    };
}

/// An installed Web Store extension's active version.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledExtension {
    /// The extension id.
    pub id: String,
    /// The active version.
    pub version: String,
    /// The manifest's name.
    pub name: String,
    /// The unpacked folder.
    pub folder: PathBuf,
}

/// Why an install failed.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum InstallError {
    /// Not an extension id.
    #[error("not an extension id")]
    BadId,
    /// Another install of the same extension is under way.
    #[error("the extension is being installed already")]
    InProgress,
    /// Another rBrowser window's install of the same extension is under way (its staging
    /// folder's pid is alive).
    #[error("an install of this extension runs in another window")]
    InProgressElsewhere,
    /// The file system.
    #[error("{0}")]
    Io(String),
    /// Not a ZIP.
    #[error("the extension's archive cannot be read: {0}")]
    Unpack(String),
    /// A member that would land outside the folder.
    #[error("the archive holds a member with an unsafe path: {0}")]
    UnsafePath(String),
    /// Two members whose names differ in letter case only: one file on Windows.
    #[error("the archive holds two members whose names differ in letter case only: {0}")]
    NameCollision(String),
    /// More members than the limit.
    #[error("the archive holds more than {0} files")]
    TooManyFiles(usize),
    /// More bytes than the limit.
    #[error("the archive unpacks to more than {0} bytes")]
    TooLarge(u64),
    /// The manifest names a key that is not the developer's.
    #[error("the manifest's key is not the key the extension is signed with")]
    KeyMismatch,
    /// The unpacked folder is no extension rBrowser loads.
    #[error(transparent)]
    Manifest(#[from] FolderError),
}

impl InstallError {
    /// The stable code of the error (MCP answers, notices).
    pub fn code(&self) -> &'static str {
        match self {
            Self::BadId => "extensions.download_bad_id",
            Self::InProgress => "extensions.install_in_progress",
            Self::InProgressElsewhere => "extensions.install_in_other_window",
            Self::Io(_) => "extensions.install_io",
            Self::Unpack(_) => "extensions.install_unpack",
            Self::UnsafePath(_) => "extensions.install_unsafe_path",
            Self::NameCollision(_) => "extensions.install_name_collision",
            Self::TooManyFiles(_) => "extensions.install_too_many_files",
            Self::TooLarge(_) => "extensions.install_too_large",
            Self::KeyMismatch => "extensions.install_key_mismatch",
            Self::Manifest(error) => error.code(),
        }
    }
}

/// The pointer file.
#[derive(Debug, Deserialize, Serialize)]
struct Pointer {
    version: String,
}

/// The folder of the Web Store extensions.
#[derive(Clone, Debug)]
pub struct ExtensionStore {
    root: PathBuf,
}

impl ExtensionStore {
    /// A store at `root`.
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    /// `%LOCALAPPDATA%\rBrowser\extensions` (`$XDG_STATE_HOME/rBrowser/extensions` elsewhere;
    /// the portable folder's when portable).
    pub fn default_root() -> Option<PathBuf> {
        rb_config::local_dir().map(|dir| dir.join("extensions"))
    }

    /// The root.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Every installed extension's active version, by id.
    pub fn installed(&self) -> Vec<InstalledExtension> {
        let Ok(entries) = fs::read_dir(&self.root) else {
            return Vec::new();
        };
        let mut ids: Vec<String> = entries
            .flatten()
            .filter_map(|entry| entry.file_name().into_string().ok())
            .filter(|name| is_id(name))
            .collect();
        ids.sort();
        ids.iter().filter_map(|id| self.installed_one(id)).collect()
    }

    /// The active version of `id`, when its pointer names a folder with a loadable manifest.
    pub fn installed_one(&self, id: &str) -> Option<InstalledExtension> {
        if !is_id(id) {
            return None;
        }
        let dir = self.root.join(id);
        let text = fs::read_to_string(dir.join(POINTER)).ok()?;
        let pointer: Pointer = serde_json::from_str(&text).ok()?;
        if !super::manifest::is_valid_version(&pointer.version) {
            return None;
        }
        let folder = dir.join(&pointer.version);
        let found = check(&folder.to_string_lossy()).ok()?;
        Some(InstalledExtension {
            id: id.to_owned(),
            version: pointer.version,
            name: found.manifest.name,
            folder,
        })
    }

    /// Installs `verified` (module documentation) with the default limits.
    ///
    /// # Errors
    ///
    /// Any step's, or another install of the id under way; nothing new is active then and the
    /// staging is gone.
    pub fn install(&self, verified: &Verified<'_>) -> Result<InstalledExtension, InstallError> {
        self.install_limited(verified, Limits::DEFAULT)
    }

    /// [`ExtensionStore::install`] with `limits`.
    ///
    /// # Errors
    ///
    /// As [`ExtensionStore::install`].
    pub fn install_limited(
        &self,
        verified: &Verified<'_>,
        limits: Limits,
    ) -> Result<InstalledExtension, InstallError> {
        if !is_id(&verified.id) {
            return Err(InstallError::BadId);
        }
        let _taken =
            InProgress::take(self.root.join(&verified.id)).ok_or(InstallError::InProgress)?;
        let staging_root = self.root.join(STAGING);
        fs::create_dir_all(&staging_root).map_err(io)?;
        let attempt = attempt_name(&verified.id);
        let staging = staging_root.join(&attempt);
        let _ = fs::remove_dir_all(&staging);
        let result = self.install_staged(verified, limits, &staging, &attempt);
        if let Err(error) = fs::remove_dir_all(&staging)
            && error.kind() != std::io::ErrorKind::NotFound
        {
            tracing::warn!(%error, id = %verified.id, "the staging folder of an extension stays");
        }
        result
    }

    fn install_staged(
        &self,
        verified: &Verified<'_>,
        limits: Limits,
        staging: &Path,
        attempt: &str,
    ) -> Result<InstalledExtension, InstallError> {
        fs::create_dir_all(staging).map_err(io)?;
        unpack(verified.zip, staging, limits)?;
        let version = write_key(staging, &verified.public_key, &verified.id)?;
        let found = check(&staging.to_string_lossy())?;

        let dir = self.root.join(&verified.id);
        fs::create_dir_all(&dir).map_err(io)?;
        let target = dir.join(&version);
        if self
            .installed_one(&verified.id)
            .is_some_and(|installed| installed.version == version)
        {
            return Ok(InstalledExtension {
                id: verified.id.clone(),
                version,
                name: found.manifest.name,
                folder: target,
            });
        }
        // A folder of this version that is not active goes aside by rename, so nothing is
        // deleted before the new folder is in place.
        let aside = dir.join(format!(".retired-{version}-{attempt}"));
        let retired = if target.exists() {
            fs::rename(&target, &aside).map_err(io)?;
            true
        } else {
            false
        };
        let restore = || {
            if retired {
                let _ = fs::rename(&aside, &target);
            }
        };
        if let Err(error) = fs::rename(staging, &target) {
            restore();
            return Err(io(error));
        }
        if let Err(error) = write_pointer(&dir, &version) {
            let _ = fs::remove_dir_all(&target);
            restore();
            return Err(error);
        }
        prune(&dir, &version);
        Ok(InstalledExtension {
            id: verified.id.clone(),
            version,
            name: found.manifest.name,
            folder: target,
        })
    }

    /// Removes `id` with every version, the pointer, its retired folders and its staging
    /// leftovers (RB-080-30); whether the extension's folder was there (a staging leftover
    /// alone is swept, but nothing was installed). What this start loaded stays loaded until
    /// the next start.
    ///
    /// # Errors
    ///
    /// Not an id, an install of the id under way in this or another window, or a file that
    /// cannot be deleted.
    pub fn remove(&self, id: &str) -> Result<bool, InstallError> {
        self.remove_with(id, &process_alive)
    }

    /// [`Self::remove`] with `alive` judging the staging names' pids: a live pid's staging of
    /// the id is another window's install, and the removal is refused with nothing deleted;
    /// a dead pid's is a leftover and goes. This process's own pid counts as dead: no install
    /// of ours runs once the folder is taken.
    pub fn remove_with(&self, id: &str, alive: &dyn Fn(u32) -> bool) -> Result<bool, InstallError> {
        if !is_id(id) {
            return Err(InstallError::BadId);
        }
        let dir = self.root.join(id);
        let _taken = InProgress::take(dir.clone()).ok_or(InstallError::InProgress)?;
        let (elsewhere, leftovers) = self.staging_of(id, alive);
        if elsewhere {
            return Err(InstallError::InProgressElsewhere);
        }
        for path in leftovers {
            if let Err(error) = fs::remove_dir_all(&path) {
                tracing::warn!(%error, "a staging folder of an extension stays: {}", path.display());
            }
        }
        if !dir.exists() {
            return Ok(false);
        }
        fs::remove_dir_all(&dir).map_err(io)?;
        Ok(true)
    }

    /// The staging folders of `id` (`<id>-<pid>-<n>`): whether one belongs to a live process
    /// other than this one, and the dead ones. A name without a pid is nobody's and stays.
    fn staging_of(&self, id: &str, alive: &dyn Fn(u32) -> bool) -> (bool, Vec<PathBuf>) {
        let Ok(entries) = fs::read_dir(self.root.join(STAGING)) else {
            return (false, Vec::new());
        };
        let mut elsewhere = false;
        let mut leftovers = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(pid) = staging_pid(&entry.file_name().to_string_lossy(), id) else {
                continue;
            };
            if !path.is_dir() {
                continue;
            }
            if pid != std::process::id() && alive(pid) {
                elsewhere = true;
            } else {
                leftovers.push(path);
            }
        }
        (elsewhere, leftovers)
    }
}

/// Writes the developer's `key` into the unpacked `manifest.json` (a key there must be the
/// same one); answers the manifest's version.
fn write_key(dir: &Path, key: &[u8], id: &str) -> Result<String, InstallError> {
    let path = dir.join(MANIFEST_FILE);
    let text = fs::read_to_string(&path).map_err(|error| match error.kind() {
        std::io::ErrorKind::NotFound => InstallError::Manifest(FolderError::NoManifest),
        _ => io(error),
    })?;
    let mut value: Value =
        serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|error| {
            FolderError::Manifest(super::manifest::ManifestError::NotJson(error.to_string()))
        })?;
    let object = value.as_object_mut().ok_or_else(|| {
        FolderError::Manifest(super::manifest::ManifestError::NotJson(
            "the top level is no object".to_owned(),
        ))
    })?;
    if let Some(written) = object.get("key") {
        let same = written
            .as_str()
            .and_then(super::manifest::id_from_key)
            .is_some_and(|found| found == id);
        if !same {
            return Err(InstallError::KeyMismatch);
        }
    }
    let encoded = base64::engine::general_purpose::STANDARD.encode(key);
    object.insert("key".to_owned(), Value::String(encoded));
    let version = object
        .get("version")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let text = serde_json::to_string_pretty(&value)
        .map_err(|error| InstallError::Io(error.to_string()))?;
    fs::write(&path, text).map_err(io)?;
    Ok(version)
}

/// Points `dir` at `version`: written beside, then renamed over the pointer.
fn write_pointer(dir: &Path, version: &str) -> Result<(), InstallError> {
    let text = serde_json::to_string(&Pointer {
        version: version.to_owned(),
    })
    .map_err(|error| InstallError::Io(error.to_string()))?;
    let temporary = dir.join(format!("{POINTER}.new"));
    fs::write(&temporary, text).map_err(io)?;
    if let Err(error) = fs::rename(&temporary, dir.join(POINTER)) {
        let _ = fs::remove_file(&temporary);
        return Err(io(error));
    }
    Ok(())
}

/// Removes every folder in `dir` but `keep`; what cannot go stays.
fn prune(dir: &Path, keep: &str) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir()
            && entry.file_name() != keep
            && let Err(error) = fs::remove_dir_all(&path)
        {
            tracing::warn!(%error, "an old extension version stays: {}", path.display());
        }
    }
}

fn io(error: std::io::Error) -> InstallError {
    InstallError::Io(error.to_string())
}

#[cfg(test)]
#[path = "store_tests.rs"]
mod tests;
