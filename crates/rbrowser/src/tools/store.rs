//! The managed tools on disk: `<root>/<tool>/<version>/` with the archive's members, and
//! `<root>/<tool>/current.json` naming the active version. The root is
//! `%LOCALAPPDATA%\rBrowser\tools` ([`ToolStore::default_root`]) - never a folder a project
//! names.
//!
//! An install ([`ToolStore::install`]) of the version that is active already does nothing. Any
//! other holds the tool's lock file `<root>/.staging/<tool>.lock` (one install per tool across
//! every rBrowser process; a second one is refused with [`ToolError::Busy`]), removes what a
//! crashed install left in the staging folder, and runs in four steps, each undone when a later
//! one fails:
//!
//! 1. **Download** into `<root>/.staging/<tool>-<pid>/`, hashed while it streams and cut off past
//!    the manifest's size. A wrong size or SHA-256 deletes the staging and activates nothing; the
//!    error names the expected and the received hash.
//! 2. **Unpack** the manifest's members, flattened to their file names (no member path can
//!    leave the staging folder), at most [`MAX_UNPACKED`] bytes together; the executable must
//!    be among them. A bare program ([`Archive::Exe`], RB-096-05) is moved to its executable's
//!    name instead.
//! 3. **Activate**: the unpacked folder is renamed to `<root>/<tool>/<version>/` - a folder of
//!    that name left over is renamed aside first, never deleted before - and the pointer replaced
//!    atomically (written beside, then renamed). The pointer before stays until then, so a
//!    failure keeps the version installed before.
//! 4. **Prune** the other versions (and what was renamed aside), then the staging.
//!
//! Blocking file and network work: the callers run it on a blocking thread.

use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use rb_util::hex;
use serde::Serialize;
use sha2::{Digest, Sha256};

use super::ToolError;
use super::fetch::Fetcher;
use super::manifest::{Archive, ToolEntry, valid_name};
use crate::versioned_dir::{activate, prune, read_pointer};

/// The staging folder below the root.
pub const STAGING: &str = ".staging";
/// The pointer file in a tool's folder.
pub use crate::versioned_dir::POINTER;
/// The most bytes one archive may unpack to (a decompression bomb stops here).
pub const MAX_UNPACKED: u64 = 1024 * 1024 * 1024;
/// The downloaded archive in the staging folder.
const ARCHIVE_FILE: &str = "download.part";
/// The read buffer of the download.
const CHUNK: usize = 64 * 1024;

/// An installed tool.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Installed {
    /// The tool.
    pub name: String,
    /// The active version.
    pub version: String,
    /// Its folder.
    pub dir: PathBuf,
    /// The program.
    pub executable: PathBuf,
}

/// The folder of the managed tools.
#[derive(Clone, Debug)]
pub struct ToolStore {
    root: PathBuf,
}

impl ToolStore {
    /// A store at `root`.
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    /// `%LOCALAPPDATA%\rBrowser\tools` (`$XDG_STATE_HOME/rBrowser/tools` elsewhere; the
    /// portable folder's when portable).
    pub fn default_root() -> Option<PathBuf> {
        rb_config::local_dir().map(|dir| dir.join("tools"))
    }

    /// The root.
    pub fn root(&self) -> &Path {
        &self.root
    }

    fn tool_dir(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }

    fn staging(&self, name: &str) -> PathBuf {
        self.root
            .join(STAGING)
            .join(format!("{name}-{}", std::process::id()))
    }

    /// The active version when it is `entry`'s.
    fn active(&self, entry: &ToolEntry) -> Option<Installed> {
        self.installed(entry)
            .filter(|installed| installed.version == entry.version)
    }

    /// The active version of `entry`'s tool, when the pointer names a plain version folder that
    /// holds the executable; any version, not only the manifest's.
    pub fn installed(&self, entry: &ToolEntry) -> Option<Installed> {
        let tool = self.tool_dir(&entry.name);
        let version = read_pointer(&tool)?;
        if !valid_name(&version) {
            tracing::warn!("the pointer of {} names no plain version", entry.name);
            return None;
        }
        let dir = tool.join(&version);
        let executable = dir.join(&entry.executable);
        executable.is_file().then(|| Installed {
            name: entry.name.clone(),
            version,
            dir,
            executable,
        })
    }

    /// Downloads, checks, unpacks and activates `entry` (module documentation).
    ///
    /// # Errors
    ///
    /// Any step's; nothing new is active then and the staging is gone.
    pub fn install(
        &self,
        fetcher: &dyn Fetcher,
        entry: &ToolEntry,
    ) -> Result<Installed, ToolError> {
        entry.validate().map_err(ToolError::Manifest)?;
        if let Some(installed) = self.active(entry) {
            return Ok(installed);
        }
        let staging_root = self.root.join(STAGING);
        fs::create_dir_all(&staging_root).map_err(io)?;
        let lock = File::options()
            .create(true)
            .truncate(false)
            .write(true)
            .open(staging_root.join(format!("{}.lock", entry.name)))
            .map_err(io)?;
        match lock.try_lock() {
            Ok(()) => {}
            Err(std::fs::TryLockError::WouldBlock) => {
                return Err(ToolError::Busy {
                    tool: entry.name.clone(),
                });
            }
            Err(std::fs::TryLockError::Error(error)) => return Err(io(error)),
        }
        // Another process may have installed it while this one waited for nothing.
        if let Some(installed) = self.active(entry) {
            return Ok(installed);
        }
        clean_staging(&staging_root, &entry.name);
        let staging = self.staging(&entry.name);
        let result = self.install_staged(fetcher, entry, &staging);
        if let Err(error) = fs::remove_dir_all(&staging)
            && error.kind() != std::io::ErrorKind::NotFound
        {
            tracing::warn!(%error, "the staging folder of {} stays", entry.name);
        }
        drop(lock);
        result
    }

    fn install_staged(
        &self,
        fetcher: &dyn Fetcher,
        entry: &ToolEntry,
        staging: &Path,
    ) -> Result<Installed, ToolError> {
        let unpacked = staging.join(&entry.version);
        fs::create_dir_all(&unpacked).map_err(io)?;
        let archive = staging.join(ARCHIVE_FILE);
        download_verified(fetcher, entry, &archive)?;
        match entry.archive {
            Archive::Zip => {
                unpack(entry, &archive, &unpacked)?;
                fs::remove_file(&archive).map_err(io)?;
            }
            // The checked download is the program: validated names keep it in the folder.
            Archive::Exe => fs::rename(&archive, unpacked.join(&entry.executable)).map_err(io)?,
        }

        let tool = self.tool_dir(&entry.name);
        fs::create_dir_all(&tool).map_err(io)?;
        // A folder of this version that is not active (the pointer names another or none) goes
        // aside first and comes back on a failure (`versioned_dir`).
        let tag = std::process::id().to_string();
        let target = activate(&tool, &entry.version, &unpacked, &tag).map_err(io)?;
        prune(&tool, &entry.version);
        let executable = target.join(&entry.executable);
        Ok(Installed {
            name: entry.name.clone(),
            version: entry.version.clone(),
            dir: target,
            executable,
        })
    }

    /// Removes `name` with every version; whether there was anything.
    ///
    /// # Errors
    ///
    /// A file that cannot be deleted (a running program).
    pub fn remove(&self, name: &str) -> Result<bool, ToolError> {
        if !valid_name(name) {
            return Err(ToolError::Manifest(format!("{name:?} is no tool name")));
        }
        let tool = self.tool_dir(name);
        if !tool.exists() {
            return Ok(false);
        }
        fs::remove_dir_all(&tool).map_err(io)?;
        Ok(true)
    }
}

/// Streams `entry`'s archive to `path`, hashing it, and refuses another size or digest.
fn download_verified(
    fetcher: &dyn Fetcher,
    entry: &ToolEntry,
    path: &Path,
) -> Result<(), ToolError> {
    let failed = |reason: String| ToolError::Download {
        tool: entry.name.clone(),
        reason,
    };
    let mut download = fetcher.open(&entry.url).map_err(failed)?;
    if let Some(length) = download.length
        && length != entry.size
    {
        return Err(failed(format!(
            "the server announces {length} bytes, the manifest pins {}",
            entry.size
        )));
    }
    let mut file = File::create(path).map_err(io)?;
    let mut hasher = Sha256::new();
    let mut total: u64 = 0;
    let mut buffer = vec![0_u8; CHUNK];
    loop {
        let read = download
            .body
            .read(&mut buffer)
            .map_err(|error| failed(error.to_string()))?;
        if read == 0 {
            break;
        }
        total = total.saturating_add(u64::try_from(read).unwrap_or(u64::MAX));
        if total > entry.size {
            return Err(failed(format!(
                "the body is larger than the {} bytes the manifest pins",
                entry.size
            )));
        }
        let chunk = buffer.get(..read).unwrap_or_default();
        hasher.update(chunk);
        file.write_all(chunk).map_err(io)?;
    }
    if total != entry.size {
        return Err(failed(format!(
            "the body is {total} bytes, the manifest pins {}",
            entry.size
        )));
    }
    let actual = hex(hasher.finalize().as_slice());
    if actual != entry.sha256 {
        return Err(ToolError::HashMismatch {
            tool: entry.name.clone(),
            version: entry.version.clone(),
            expected: entry.sha256.clone(),
            actual,
        });
    }
    file.sync_all().map_err(io)
}

/// Unpacks `entry`'s members from `archive` into `dir`, flattened.
fn unpack(entry: &ToolEntry, archive: &Path, dir: &Path) -> Result<(), ToolError> {
    let unpack_failed = |reason: String| ToolError::Unpack {
        tool: entry.name.clone(),
        reason,
    };
    let file = File::open(archive).map_err(io)?;
    let mut zip = zip::ZipArchive::new(file).map_err(|error| unpack_failed(error.to_string()))?;
    let mut budget = MAX_UNPACKED;
    for index in 0..zip.len() {
        let mut member = zip
            .by_index(index)
            .map_err(|error| unpack_failed(error.to_string()))?;
        if !member.is_file() {
            continue;
        }
        let full = member
            .name()
            .map_err(|error| unpack_failed(error.to_string()))?
            .into_owned();
        let Some(base) = full.rsplit(['/', '\\']).next() else {
            continue;
        };
        if !entry.members.iter().any(|wanted| wanted == base) || !valid_name(base) {
            continue;
        }
        let mut out = File::create(dir.join(base)).map_err(io)?;
        let copied = std::io::copy(&mut (&mut member).take(budget.saturating_add(1)), &mut out)
            .map_err(|error| unpack_failed(error.to_string()))?;
        if copied > budget {
            return Err(unpack_failed(format!(
                "{full} unpacks to more than {MAX_UNPACKED} bytes"
            )));
        }
        budget -= copied;
    }
    if !dir.join(&entry.executable).is_file() {
        return Err(unpack_failed(format!(
            "the archive holds no {}",
            entry.executable
        )));
    }
    Ok(())
}

/// Removes what a crashed install of `name` left in `staging`: its folder of an older layout
/// (`<name>`) and of any process (`<name>-<pid>`). Called under the tool's lock, so none of them
/// belongs to an install that still runs.
fn clean_staging(staging: &Path, name: &str) {
    let Ok(entries) = fs::read_dir(staging) else {
        return;
    };
    let prefix = format!("{name}-");
    for entry in entries.flatten() {
        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();
        let ours = file_name == name
            || file_name
                .strip_prefix(&prefix)
                .is_some_and(|pid| !pid.is_empty() && pid.bytes().all(|b| b.is_ascii_digit()));
        if ours
            && entry.path().is_dir()
            && let Err(error) = fs::remove_dir_all(entry.path())
        {
            tracing::warn!(%error, "a stale staging folder stays: {}", entry.path().display());
        }
    }
}

/// `bytes` as lowercase hex.
pub fn sha256_hex(bytes: &[u8]) -> String {
    hex(Sha256::digest(bytes).as_slice())
}

fn io(error: std::io::Error) -> ToolError {
    ToolError::Io(error.to_string())
}

#[cfg(test)]
#[path = "store_tests.rs"]
mod tests;
