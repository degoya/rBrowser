//! The log file that rotates by size: `rbrowser.log` is written until the next line would take it
//! past [`RotationPolicy::max_bytes`]; then it becomes `rbrowser.1.log`, the older archives move
//! up one number, and the one past [`RotationPolicy::max_files`] is deleted.
//!
//! `tracing-appender` rotates by time only, so the size rule lives here; its non-blocking writer
//! still carries the lines to this file off the caller's thread (`logging`).

use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

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

/// The name of archive `index`: `rbrowser.log` -> `rbrowser.<index>.log`.
pub fn rotated_name(file_name: &str, index: usize) -> String {
    match file_name.rsplit_once('.') {
        Some((stem, extension)) if !stem.is_empty() => format!("{stem}.{index}.{extension}"),
        _ => format!("{file_name}.{index}"),
    }
}

/// A file that rotates by size, see the module.
#[derive(Debug)]
pub struct RotatingFile {
    dir: PathBuf,
    file_name: String,
    policy: RotationPolicy,
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
            policy: RotationPolicy {
                max_bytes: policy.max_bytes.max(1),
                max_files: policy.max_files.max(1),
            },
            file,
            written,
        })
    }

    /// The path of the file being written.
    pub fn path(&self) -> PathBuf {
        self.dir.join(&self.file_name)
    }

    fn rotate(&mut self) -> io::Result<()> {
        self.file.flush()?;
        let current = self.path();
        let keep = self.policy.max_files - 1;
        if keep == 0 {
            self.file = OpenOptions::new()
                .write(true)
                .truncate(true)
                .open(&current)?;
            self.written = 0;
            return Ok(());
        }
        let archive = |index| self.dir.join(rotated_name(&self.file_name, index));
        remove_if_present(&archive(keep))?;
        for index in (1..keep).rev() {
            rename_if_present(&archive(index), &archive(index + 1))?;
        }
        // The handle stays open across the rename: std opens files with FILE_SHARE_DELETE on
        // Windows, so the rename succeeds there too, and the new handle replaces it below.
        std::fs::rename(&current, archive(1))?;
        self.file = append(&current)?;
        self.written = 0;
        Ok(())
    }
}

impl Write for RotatingFile {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let incoming = u64::try_from(buf.len()).unwrap_or(u64::MAX);
        if self.written > 0 && self.written.saturating_add(incoming) > self.policy.max_bytes {
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

fn append(path: &Path) -> io::Result<File> {
    OpenOptions::new().create(true).append(true).open(path)
}

fn remove_if_present(path: &Path) -> io::Result<()> {
    match std::fs::remove_file(path) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
        _ => Ok(()),
    }
}

fn rename_if_present(from: &Path, to: &Path) -> io::Result<()> {
    match std::fs::rename(from, to) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
        _ => Ok(()),
    }
}

#[cfg(test)]
#[path = "rotate_tests.rs"]
mod tests;
