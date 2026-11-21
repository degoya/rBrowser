//! A file written whole or not at all (RB-094-10): the bytes go to a hidden temporary file
//! beside the target (`.<name>.<pid>-<n>.tmp`, unique per process and write), are synced to the
//! disk, and the file is renamed over the target, so a reader or a crash sees the old file or
//! the new one, never half of it. A failed write removes its temporary file.

use std::fs::OpenOptions;
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// Writes `bytes` to `path` whole, replacing the file there.
///
/// # Errors
///
/// `path` has no file name, or the temporary file cannot be written or renamed.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    write_through(path, bytes, false)
}

/// [`write_atomic`] for a file only its owner may read (mode `0600` on Unix; on Windows the
/// folder's ACL decides): an instance file with its token, the session state.
///
/// # Errors
///
/// As [`write_atomic`].
pub fn write_atomic_private(path: &Path, bytes: &[u8]) -> io::Result<()> {
    write_through(path, bytes, true)
}

fn write_through(path: &Path, bytes: &[u8], private: bool) -> io::Result<()> {
    let temporary = temporary_path(path)?;
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        if private {
            options.mode(0o600);
        }
    }
    #[cfg(not(unix))]
    let _ = private;
    let written = options
        .open(&temporary)
        .and_then(|mut file| {
            file.write_all(bytes)?;
            file.sync_all()
        })
        .and_then(|()| std::fs::rename(&temporary, path));
    if written.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    written
}

/// The temporary file beside `path` one write goes through.
fn temporary_path(path: &Path) -> io::Result<PathBuf> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let name = path
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "the path names no file"))?;
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    let mut temporary = std::ffi::OsString::from(".");
    temporary.push(name);
    temporary.push(format!(".{}-{n}.tmp", std::process::id()));
    Ok(path.with_file_name(temporary))
}

#[cfg(test)]
#[path = "fs_tests.rs"]
mod tests;
