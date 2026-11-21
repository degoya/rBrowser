//! rBrowser's own reverses on disk (RB-096-12): what each running process set, so a reverse of
//! a run that ended without cleaning up (a crash, a kill) is known for what it was. An entry is a
//! device (a hash of its serial, never the serial) and a port; each process writes a file of its
//! own, `<local folder>/android-reverses/<pid>.json`, and only reads the others' - two processes
//! never race on one file (code review w33, item 8).
//!
//! A run's leftovers are dropped from the ledger at the engine's start ([`Ledger::sweep`]), never
//! removed from the device: adb names no owner of a reverse, so a `tcp:P tcp:P` listed after a
//! crash may by now be another tool's (Metro's 8081 after an `adb reverse --remove-all`). Such a
//! reverse of rBrowser stands until the device reconnects or the person removes it; the debug log
//! names its port. A file that does not read is moved aside (`.broken`), never overwritten.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest as _, Sha256};

use crate::cdp::lock;

/// The ledger's folder in the local folder.
pub const FOLDER_NAME: &str = "android-reverses";
/// The largest file read; a bigger one is no ledger of this app.
const MAX_BYTES: u64 = 64 * 1024;
/// The most entries one process notes (a reverse past them is set, not noted).
pub const MAX_ENTRIES: usize = 256;
/// The most files of the folder looked at.
const MAX_FILES: usize = 1024;
/// The hex digits of a device's hash.
const KEY_DIGITS: usize = 16;

/// One reverse rBrowser set.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Entry {
    /// [`device_key`] of the serial.
    pub device: String,
    /// The port, the same on both sides.
    pub port: u16,
    /// The process that set it (its file's name).
    pub pid: u32,
}

impl Entry {
    /// An entry.
    pub fn new(device: &str, port: u16, pid: u32) -> Self {
        Self {
            device: device.to_owned(),
            port,
            pid,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
struct Noted {
    device: String,
    port: u16,
}

#[derive(Default, Serialize, Deserialize)]
struct File {
    version: u32,
    reverses: Vec<Noted>,
}

/// A device as the ledger names it: the start of its serial's SHA-256.
pub fn device_key(serial: &str) -> String {
    let mut key = rb_util::hex(Sha256::digest(serial.as_bytes()).as_slice());
    key.truncate(KEY_DIGITS);
    key
}

/// The ledger; without a folder (no local folder, the tests) it keeps nothing.
#[derive(Clone, Debug)]
pub struct Ledger {
    folder: Option<PathBuf>,
    /// This process's entries, the whole of its file.
    own: Arc<Mutex<BTreeSet<Noted>>>,
}

impl Ledger {
    /// The ledger in `folder`.
    pub fn at(folder: Option<PathBuf>) -> Self {
        Self {
            folder,
            own: Arc::default(),
        }
    }

    /// The ledger in the app's local folder.
    pub fn default_place() -> Self {
        Self::at(rb_config::local_dir().map(|dir| dir.join(FOLDER_NAME)))
    }

    /// Every entry of every process; a file that does not read gives none.
    pub fn entries(&self) -> Vec<Entry> {
        self.files()
            .into_iter()
            .filter_map(|(pid, path)| read(&path).ok().map(|noted| entries(pid, noted)))
            .flatten()
            .collect()
    }

    /// Notes that this process set `port` on `serial`.
    pub fn add(&self, serial: &str, port: u16) {
        let noted = Noted {
            device: device_key(serial),
            port,
        };
        let mut own = lock(&self.own);
        if own.contains(&noted) {
            return;
        }
        if own.len() >= MAX_ENTRIES {
            tracing::debug!(
                port,
                "the ledger of adb reverses is full; the reverse is not noted"
            );
            return;
        }
        own.insert(noted);
        self.write_own(&own);
    }

    /// Forgets that this process set `port` on `serial`.
    pub fn remove(&self, serial: &str, port: u16) {
        let noted = Noted {
            device: device_key(serial),
            port,
        };
        let mut own = lock(&self.own);
        if own.remove(&noted) {
            self.write_own(&own);
        }
    }

    /// Drops what no running rBrowser noted: the files of processes `alive` says ended, and this
    /// process's entries it did not note itself (a run before with the same pid). The entries
    /// dropped are answered and logged; their reverses are left on the device.
    pub fn sweep(&self, alive: impl Fn(u32) -> bool) -> Vec<Entry> {
        let me = std::process::id();
        let mut dropped = Vec::new();
        for (pid, path) in self.files() {
            if pid != me && alive(pid) {
                continue;
            }
            let own = lock(&self.own);
            match read(&path) {
                Ok(noted) => {
                    let stale = noted
                        .into_iter()
                        .filter(|noted| pid != me || !own.contains(noted));
                    dropped.extend(entries(pid, stale));
                    if pid == me {
                        self.write_own(&own);
                    } else if let Err(error) = std::fs::remove_file(&path) {
                        tracing::debug!(kind = %error.kind(), "a ledger file of adb reverses stays");
                    }
                }
                Err(()) => set_aside(&path),
            }
        }
        for entry in &dropped {
            tracing::debug!(
                device = %entry.device,
                port = entry.port,
                "an adb reverse of an earlier run may still stand; left on the device"
            );
        }
        dropped
    }

    /// This process's file.
    fn own_path(&self) -> Option<PathBuf> {
        let folder = self.folder.as_ref()?;
        Some(folder.join(format!("{}.json", std::process::id())))
    }

    /// Writes `own` as this process's file; none when empty.
    fn write_own(&self, own: &BTreeSet<Noted>) {
        let Some(path) = self.own_path() else {
            return;
        };
        let written = if own.is_empty() {
            match std::fs::remove_file(&path) {
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
                other => other,
            }
        } else {
            let file = File {
                version: 1,
                reverses: own.iter().cloned().collect(),
            };
            serde_json::to_vec_pretty(&file)
                .map_err(std::io::Error::other)
                .and_then(|bytes| {
                    if let Some(folder) = path.parent() {
                        std::fs::create_dir_all(folder)?;
                    }
                    rb_util::fs::write_atomic(&path, &bytes)
                })
        };
        if let Err(error) = written {
            tracing::debug!(kind = %error.kind(), "the ledger of adb reverses was not written");
        }
    }

    /// The folder's ledger files (`<pid>.json`), at most [`MAX_FILES`].
    fn files(&self) -> Vec<(u32, PathBuf)> {
        let Some(folder) = &self.folder else {
            return Vec::new();
        };
        let Ok(listing) = std::fs::read_dir(folder) else {
            return Vec::new();
        };
        listing
            .take(MAX_FILES)
            .filter_map(Result::ok)
            .filter_map(|item| {
                let name = item.file_name();
                let pid = name.to_str()?.strip_suffix(".json")?.parse::<u32>().ok()?;
                Some((pid, item.path()))
            })
            .collect()
    }
}

fn entries(pid: u32, noted: impl IntoIterator<Item = Noted>) -> Vec<Entry> {
    noted
        .into_iter()
        .map(|noted| Entry::new(&noted.device, noted.port, pid))
        .collect()
}

/// A ledger file's entries; `Err` when it is too big or does not read.
fn read(path: &Path) -> Result<Vec<Noted>, ()> {
    let fits = std::fs::metadata(path).is_ok_and(|meta| meta.len() <= MAX_BYTES);
    if !fits {
        return Err(());
    }
    let bytes = std::fs::read(path).map_err(|_| ())?;
    let value: Value = serde_json::from_slice(&bytes).map_err(|_| ())?;
    let file: File = serde_json::from_value(value).map_err(|_| ())?;
    Ok(file.reverses.into_iter().take(MAX_ENTRIES).collect())
}

/// Moves a file that does not read aside (`<name>.broken`), so nothing overwrites it unseen.
fn set_aside(path: &Path) {
    let mut aside = path.as_os_str().to_owned();
    aside.push(".broken");
    if let Err(error) = std::fs::rename(path, PathBuf::from(aside)) {
        tracing::debug!(kind = %error.kind(), "a broken ledger file of adb reverses stays");
    } else {
        tracing::debug!("a broken ledger file of adb reverses was moved aside");
    }
}

#[cfg(test)]
#[path = "reverse_ledger_tests.rs"]
mod tests;
