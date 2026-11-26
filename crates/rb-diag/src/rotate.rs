//! The log file that rotates by size: `rbrowser.log` is written until the next line would take it
//! past [`RotationPolicy::max_bytes`]; then it becomes `rbrowser.1.log`, the older archives move
//! up one number, and the one past [`RotationPolicy::max_files`] is deleted.
//!
//! The policy changes while the file is written ([`SharedRotation`], RB-094-28): a new size
//! applies to the next write, a smaller number of files clears the archives past it at the next
//! rotation - never the file being written.
//!
//! `tracing-appender` rotates by time only, so the size rule lives here; its non-blocking writer
//! still carries the lines to this file off the caller's thread (`logging`).

use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

/// How big the log file grows and how many files are kept.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RotationPolicy {
    /// The size at which the file rotates, in bytes.
    pub max_bytes: u64,
    /// The files kept, the one being written included (at least 1).
    pub max_files: usize,
}

impl Default for RotationPolicy {
    fn default() -> Self {
        Self::from_settings(
            crate::settings::DEFAULT_MAX_FILE_SIZE_MB,
            crate::settings::DEFAULT_MAX_FILES,
        )
    }
}

impl RotationPolicy {
    /// The policy of the settings `logs.maxFileSizeMb` and `logs.maxFiles`; each at least 1.
    pub fn from_settings(max_file_size_mb: u64, max_files: usize) -> Self {
        Self {
            max_bytes: max_file_size_mb.max(1) * 1024 * 1024,
            max_files: max_files.max(1),
        }
    }
}

/// The rotation policy of a running [`RotatingFile`], shared with whoever changes it (the
/// logger's [`crate::LogHandle`]). Cheap to clone; every clone sees the same policy.
#[derive(Clone, Debug)]
pub struct SharedRotation(Arc<Limits>);

/// The two values of a [`SharedRotation`]; a reader may see one changed before the other, which
/// only moves the rotation by one write.
#[derive(Debug)]
struct Limits {
    max_bytes: AtomicU64,
    max_files: AtomicUsize,
}

impl SharedRotation {
    /// A shared policy starting at `policy`, each value at least 1.
    pub fn new(policy: RotationPolicy) -> Self {
        let shared = Self(Arc::new(Limits {
            max_bytes: AtomicU64::new(1),
            max_files: AtomicUsize::new(1),
        }));
        shared.set(policy);
        shared
    }

    /// The policy in effect now.
    pub fn get(&self) -> RotationPolicy {
        RotationPolicy {
            max_bytes: self.0.max_bytes.load(Ordering::Relaxed),
            max_files: self.0.max_files.load(Ordering::Relaxed),
        }
    }

    /// Puts `policy` in effect for the next write, each value at least 1.
    pub fn set(&self, policy: RotationPolicy) {
        self.0
            .max_bytes
            .store(policy.max_bytes.max(1), Ordering::Relaxed);
        self.0
            .max_files
            .store(policy.max_files.max(1), Ordering::Relaxed);
    }
}

/// The name of archive `index`: `rbrowser.log` -> `rbrowser.<index>.log`.
pub fn rotated_name(file_name: &str, index: usize) -> String {
    match file_name.rsplit_once('.') {
        Some((stem, extension)) if !stem.is_empty() => format!("{stem}.{index}.{extension}"),
        _ => format!("{file_name}.{index}"),
    }
}

/// The number of `candidate` when it is an archive of `file_name` as [`rotated_name`] names it.
pub(crate) fn archive_index(file_name: &str, candidate: &str) -> Option<usize> {
    let number = match file_name.rsplit_once('.') {
        Some((stem, extension)) if !stem.is_empty() => candidate
            .strip_prefix(stem)?
            .strip_prefix('.')?
            .strip_suffix(extension)?
            .strip_suffix('.')?,
        _ => candidate.strip_prefix(file_name)?.strip_prefix('.')?,
    };
    let index: usize = number.parse().ok()?;
    (index.to_string() == number).then_some(index)
}

/// A file that rotates by size, see the module.
#[derive(Debug)]
pub struct RotatingFile {
    dir: PathBuf,
    file_name: String,
    rotation: SharedRotation,
    file: File,
    written: u64,
}

impl RotatingFile {
    /// Opens (or creates) `dir/file_name` for appending; the directory is created when missing.
    pub fn open(dir: &Path, file_name: &str, policy: RotationPolicy) -> io::Result<Self> {
        std::fs::create_dir_all(dir)?;
        let file = append(&dir.join(file_name))?;
        let written = file.metadata()?.len();
        Ok(Self {
            dir: dir.to_path_buf(),
            file_name: file_name.to_owned(),
            rotation: SharedRotation::new(policy),
            file,
            written,
        })
    }

    /// The path of the file being written.
    pub fn path(&self) -> PathBuf {
        self.dir.join(&self.file_name)
    }

    /// The policy of this file; a change applies to the next write.
    pub fn rotation(&self) -> &SharedRotation {
        &self.rotation
    }

    fn archive(&self, index: usize) -> PathBuf {
        self.dir.join(rotated_name(&self.file_name, index))
    }

    fn rotate(&mut self) -> io::Result<()> {
        self.file.flush()?;
        let current = self.path();
        let keep = self.rotation.get().max_files.max(1) - 1;
        remove_archives_from(&self.dir, &self.file_name, keep.max(1))?;
        if keep == 0 {
            self.file = OpenOptions::new()
                .write(true)
                .truncate(true)
                .open(&current)?;
            self.written = 0;
            return Ok(());
        }
        for index in (1..keep).rev() {
            rename_if_present(&self.archive(index), &self.archive(index + 1))?;
        }
        // The handle stays open across the rename: std opens files with FILE_SHARE_DELETE on
        // Windows, so the rename succeeds there too, and the new handle replaces it below.
        std::fs::rename(&current, self.archive(1))?;
        self.file = append(&current)?;
        self.written = 0;
        Ok(())
    }
}

impl Write for RotatingFile {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let incoming = u64::try_from(buf.len()).unwrap_or(u64::MAX);
        let max_bytes = self.rotation.get().max_bytes;
        if self.written > 0 && self.written.saturating_add(incoming) > max_bytes {
            self.rotate()?;
        }
        self.file.write_all(buf)?;
        self.written = self.written.saturating_add(incoming);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}

/// Deletes every archive of `dir/file_name` numbered `first` or higher - also those an earlier,
/// larger number of files left, and past a gap in the numbers.
pub(crate) fn remove_archives_from(dir: &Path, file_name: &str, first: usize) -> io::Result<()> {
    let first = first.max(1);
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name();
        let Some(index) = name
            .to_str()
            .and_then(|name| archive_index(file_name, name))
        else {
            continue;
        };
        if index < first || !entry.file_type()?.is_file() {
            continue;
        }
        if let Err(error) = std::fs::remove_file(entry.path())
            && error.kind() != io::ErrorKind::NotFound
        {
            return Err(error);
        }
    }
    Ok(())
}

fn append(path: &Path) -> io::Result<File> {
    OpenOptions::new().create(true).append(true).open(path)
}

pub(crate) fn rename_if_present(from: &Path, to: &Path) -> io::Result<()> {
    match std::fs::rename(from, to) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
        _ => Ok(()),
    }
}

#[cfg(test)]
#[path = "rotate_tests.rs"]
mod tests;
