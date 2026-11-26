//! CEF's own log file, `cef.log` (`CefSettings.log_file`), held to the size of the core's
//! (RB-099-07): Chromium appends to it from every process and never rotates it, and a warning
//! flood took it to 948 MB.
//!
//! The app trims it before CEF starts and then once a minute ([`spawn_guard`]) by the rotation
//! of `rbrowser.log` (`logs.maxFileSizeMb`, `logs.maxFiles`, live): a file past the size keeps
//! its last `max_bytes` - from a line start - in `cef.1.log`, the older archives move up one
//! number, and the file is emptied in place. CEF keeps its handle open and writes in append mode
//! (`FILE_APPEND_DATA` on Windows, `O_APPEND` elsewhere), so its next line lands at the start of
//! the emptied file; a line written between the copy and the emptying is lost. Renaming the
//! open file is no option: Chromium opens it without `FILE_SHARE_DELETE` on Windows.
//!
//! The diagnostics ZIP takes CEF's logs with their last [`BUNDLE_TAIL_BYTES`] only ([`tail`]).

use std::fs::{File, OpenOptions};
use std::io::{self, BufRead as _, BufReader, Read as _, Seek as _, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::paths::CEF_LOG_FILE_NAME;
use crate::rotate::{
    RotationPolicy, SharedRotation, archive_index, remove_archives_from, rename_if_present,
    rotated_name,
};

/// The end of a CEF log the diagnostics ZIP takes: 5 MiB.
pub const BUNDLE_TAIL_BYTES: u64 = 5 * 1024 * 1024;

/// How often the guard looks at the size of CEF's log while the app runs.
pub const CHECK_EVERY: Duration = Duration::from_secs(60);

/// Whether `name` is CEF's log (`cef.log`) or one of its archives (`cef.1.log`, ...).
pub fn is_cef_log(name: &str) -> bool {
    name == CEF_LOG_FILE_NAME || archive_index(CEF_LOG_FILE_NAME, name).is_some()
}

/// Trims `dir/file_name` when it is larger than `policy.max_bytes` (see the module): its end goes
/// to archive 1, unless only one file is kept, and the file is emptied in place. `true` when it
/// was trimmed; a missing file is left alone.
pub fn trim(dir: &Path, file_name: &str, policy: RotationPolicy) -> io::Result<bool> {
    let path = dir.join(file_name);
    let len = match std::fs::metadata(&path) {
        Ok(meta) => meta.len(),
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error),
    };
    if len <= policy.max_bytes.max(1) {
        return Ok(false);
    }
    let keep = policy.max_files.max(1) - 1;
    remove_archives_from(dir, file_name, keep.max(1))?;
    if keep > 0 {
        for index in (1..keep).rev() {
            rename_if_present(
                &dir.join(rotated_name(file_name, index)),
                &dir.join(rotated_name(file_name, index + 1)),
            )?;
        }
        let (mut end, _) = open_tail(&path, policy.max_bytes)?;
        let mut archive = File::create(dir.join(rotated_name(file_name, 1)))?;
        io::copy(&mut end, &mut archive)?;
    }
    OpenOptions::new().write(true).open(&path)?.set_len(0)?;
    Ok(true)
}

/// The end of the file at `path`: at most `max` bytes, from the first line start in them, and
/// the number of bytes before it.
pub fn tail(path: &Path, max: u64) -> io::Result<(Vec<u8>, u64)> {
    let (mut end, left_out) = open_tail(path, max)?;
    let mut bytes = Vec::new();
    end.read_to_end(&mut bytes)?;
    Ok((bytes, left_out))
}

/// A reader at the first line start of the last `max` bytes of `path`, and where it starts.
fn open_tail(path: &Path, max: u64) -> io::Result<(BufReader<File>, u64)> {
    let mut file = File::open(path)?;
    let len = file.metadata()?.len();
    let mut start = len.saturating_sub(max);
    if start == 0 {
        return Ok((BufReader::new(file), 0));
    }
    // Starts the tail exactly at a line? Then nothing of it is skipped.
    file.seek(SeekFrom::Start(start - 1))?;
    let mut before = [0_u8; 1];
    file.read_exact(&mut before)?;
    let mut reader = BufReader::new(file);
    if before[0] != b'\n' {
        let mut partial = Vec::new();
        let skipped = reader.read_until(b'\n', &mut partial)?;
        start = start.saturating_add(u64::try_from(skipped).unwrap_or(u64::MAX));
    }
    Ok((reader, start))
}

/// Starts the thread that trims CEF's log in `dir` every `every` by `rotation` - the core log's
/// policy, so a change in the settings applies at the next look. It runs while `dir` exists - for
/// the app, the life of the process; a failed trim is logged and tried again at the next look.
pub fn spawn_guard(
    dir: PathBuf,
    rotation: SharedRotation,
    every: Duration,
) -> io::Result<std::thread::JoinHandle<()>> {
    std::thread::Builder::new()
        .name("rb-cef-log".to_owned())
        .spawn(move || {
            loop {
                std::thread::sleep(every);
                // A folder that is gone holds no log to keep small (and ends a test's guard).
                if !dir.is_dir() {
                    break;
                }
                match trim(&dir, CEF_LOG_FILE_NAME, rotation.get()) {
                    Ok(true) => tracing::info!(
                        target: crate::logging::target::CORE,
                        max_bytes = rotation.get().max_bytes,
                        "CEF's log reached its size; its end went to its first archive"
                    ),
                    Ok(false) => {}
                    Err(error) => tracing::warn!(
                        target: crate::logging::target::CORE,
                        %error,
                        "CEF's log could not be trimmed"
                    ),
                }
            }
        })
}

#[cfg(test)]
#[path = "cef_log_tests.rs"]
mod tests;
