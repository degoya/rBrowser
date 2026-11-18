//! The two locks of an update, released by the operating system with the process - never a
//! process id that may have been reused.
//!
//! * [`UpdaterLock`] on `<data>/update/updater.lock`: held by the updater for its whole life;
//!   whether an updater runs is whether it can be taken.
//! * [`InstanceLock`] on `<data>/update/instance.lock`: every window holds it shared while it
//!   runs; the updater waits until it can take it alone, which is when the last window ended.
//!   Beside it each window holds a marker of its own in `<data>/update/windows/` alone, so a
//!   window can count the others ([`InstanceLock::others`]) before it hands an update over
//!   (RB-090-14): a shared lock says that someone holds it, not how many.
//!
//! Copied and adapted from rCommander's `crates/rc-update/src/install/locks.rs` (RB-090-02):
//! without the database lock.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};

use super::{INSTANCE_LOCK_FILE, LOCK_FILE, update_dir};

fn open(data: &Path, name: &str) -> Result<fs::File> {
    let directory = update_dir(data);
    fs::create_dir_all(&directory).with_context(|| format!("create {}", directory.display()))?;
    let path = directory.join(name);
    fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)
        .with_context(|| format!("open {}", path.display()))
}

/// The updater's lock, released with the process.
#[derive(Debug)]
pub struct UpdaterLock {
    _file: fs::File,
}

impl UpdaterLock {
    /// Takes the lock; `None` while another updater holds it.
    ///
    /// # Errors
    ///
    /// When the lock file cannot be opened or locked.
    pub fn acquire(data: &Path) -> Result<Option<Self>> {
        let file = open(data, LOCK_FILE)?;
        match file.try_lock() {
            Ok(()) => Ok(Some(Self { _file: file })),
            Err(fs::TryLockError::WouldBlock) => Ok(None),
            Err(fs::TryLockError::Error(error)) => Err(error).context("lock the updater"),
        }
    }
}

/// Whether an updater runs for this data directory right now.
#[must_use]
pub fn updater_running(data: &Path) -> bool {
    if !update_dir(data).join(LOCK_FILE).exists() {
        return false;
    }
    matches!(UpdaterLock::acquire(data), Ok(None))
}

/// Inside [`super::UPDATE_DIR`]: one marker per running window, each locked by its window.
pub const WINDOWS_DIR: &str = "windows";

/// A window's share of the instance lock and its own marker, held for its life.
#[derive(Debug)]
pub struct InstanceLock {
    _file: fs::File,
    /// This window's marker, locked alone, removed when the window lets go.
    marker: Option<(fs::File, PathBuf)>,
}

impl Drop for InstanceLock {
    fn drop(&mut self) {
        if let Some((file, path)) = self.marker.take() {
            // Closed first: Windows removes no file that is open.
            drop(file);
            let _ = fs::remove_file(path);
        }
    }
}

/// This window's marker, locked alone; `None` when it cannot be made (counting then sees one
/// window fewer, which is never this one).
fn marker(data: &Path) -> Option<(fs::File, PathBuf)> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let directory = update_dir(data).join(WINDOWS_DIR);
    fs::create_dir_all(&directory).ok()?;
    let name = format!(
        "{}-{}.lock",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    );
    let path = directory.join(name);
    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)
        .ok()?;
    file.try_lock().ok()?;
    Some((file, path))
}

impl InstanceLock {
    /// Takes a share of the lock for this window. `None` while an updater holds it alone (it
    /// lets go within moments, once it saw the old window end): the window runs without one.
    ///
    /// # Errors
    ///
    /// When the lock file cannot be opened or locked.
    pub fn share(data: &Path) -> Result<Option<Self>> {
        let file = open(data, INSTANCE_LOCK_FILE)?;
        match file.try_lock_shared() {
            Ok(()) => Ok(Some(Self {
                _file: file,
                marker: marker(data),
            })),
            Err(fs::TryLockError::WouldBlock) => Ok(None),
            Err(fs::TryLockError::Error(error)) => Err(error).context("lock the instance"),
        }
    }

    /// How many other windows of this data folder run now. A marker nobody holds is a window
    /// that ended without letting go (a crash): it is removed and not counted; one that cannot be
    /// read is counted, so an update never starts against a window it could not see.
    #[must_use]
    pub fn others(&self) -> usize {
        let Some((_, own)) = &self.marker else {
            return 0;
        };
        let Some(directory) = own.parent() else {
            return 0;
        };
        let Ok(entries) = fs::read_dir(directory) else {
            return 0;
        };
        let mut running = 0;
        for path in entries.flatten().map(|entry| entry.path()) {
            if &path == own || path.extension().is_none_or(|extension| extension != "lock") {
                continue;
            }
            let Ok(file) = fs::OpenOptions::new().read(true).write(true).open(&path) else {
                running += 1;
                continue;
            };
            match file.try_lock() {
                Ok(()) => {
                    let _ = file.unlock();
                    drop(file);
                    let _ = fs::remove_file(&path);
                }
                Err(_) => running += 1,
            }
        }
        running
    }

    /// Waits up to `wait` until no window holds a share; whether none does. The lock is let go
    /// again at once: the new version takes its own share when it starts.
    ///
    /// # Errors
    ///
    /// When the lock file cannot be opened or locked.
    pub fn wait_until_free(data: &Path, wait: Duration) -> Result<bool> {
        let file = open(data, INSTANCE_LOCK_FILE)?;
        let started = Instant::now();
        loop {
            match file.try_lock() {
                Ok(()) => {
                    file.unlock().context("unlock the instance")?;
                    return Ok(true);
                }
                Err(fs::TryLockError::WouldBlock) if started.elapsed() < wait => {
                    std::thread::sleep(Duration::from_millis(200));
                }
                Err(fs::TryLockError::WouldBlock) => return Ok(false),
                Err(fs::TryLockError::Error(error)) => {
                    return Err(error).context("lock the instance");
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "locks_tests.rs"]
mod tests;
