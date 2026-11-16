//! One export or conversion (RB-090-52): [`plan`] holds the request to the Videos panel's rules -
//! a video rBrowser recorded (registered, of its stamp, no link on the way), not the recording
//! being written, a result not there yet - and [`make`] runs ffmpeg below the app's priority
//! into `<target>.part`, renames it once complete, writes the breakpoint's poster where there is
//! none yet and registers what it wrote, so the route serves it and the panel deletes it like
//! the recorder's own files. A run that fails or is cancelled (the app's end) leaves nothing.

use std::collections::VecDeque;
use std::ffi::OsString;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use rb_ipc::VideoTarget;
use tokio::io::{AsyncBufReadExt as _, BufReader};
use tokio::sync::watch;

use super::convert::{
    ConvertError, Encoder, Progress, convert_args, parse_duration, parse_progress, part_name,
    percent, target_name,
};
use super::library::{self, LibraryError};
use super::poster::{self, poster_name};
use super::registry::Recorded;

/// A run that says nothing for this long hangs and is stopped (`-progress` writes twice a
/// second).
pub const STALL_TIMEOUT: Duration = Duration::from_secs(120);
/// The error of a cancelled run.
pub const CANCELLED: &str = "the conversion was cancelled";
/// The lines of ffmpeg's error output kept for the error.
const ERROR_LINES: usize = 8;

/// How a result is encoded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EncodeWith {
    /// The encoder.
    pub encoder: Encoder,
    /// 1 to 100.
    pub quality: u32,
    /// The encoder's threads.
    pub threads: u32,
}

/// One planned export or conversion.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Job {
    /// The recording's id.
    pub recording: String,
    /// The video it is made from, by name.
    pub file: String,
    /// The name of the result.
    pub target: String,
    /// What it is made into.
    pub to: VideoTarget,
    /// The recording's folder.
    pub folder: PathBuf,
    /// The video's path.
    pub source: PathBuf,
}

/// Plans making video `file` of recording `id` in the project at `root` into `to`.
///
/// # Errors
///
/// The recording is in `running`; the file is no video of `to`'s other format, not one
/// rBrowser recorded, changed or behind a link ([`ConvertError::Library`]); the result is
/// there already ([`ConvertError::Exists`]).
pub fn plan(
    root: &Path,
    recorded: &Recorded,
    id: &str,
    file: &str,
    to: VideoTarget,
    running: &[&str],
) -> Result<Job, ConvertError> {
    if running.contains(&id) {
        return Err(LibraryError::Running.into());
    }
    let target = target_name(file, to)?;
    let source = library::locate(root, recorded, id, Some(file))?;
    let folder = source.parent().ok_or(LibraryError::NotFound)?.to_path_buf();
    match std::fs::symlink_metadata(folder.join(&target)) {
        Ok(_) => return Err(ConvertError::Exists(target)),
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(error) => return Err(LibraryError::Io(error.to_string()).into()),
    }
    Ok(Job {
        recording: id.to_owned(),
        file: file.to_owned(),
        target,
        to,
        folder,
        source,
    })
}

fn io(path: &Path, error: &std::io::Error) -> ConvertError {
    ConvertError::Library(LibraryError::Io(format!("{}: {error}", path.display())))
}

/// Removes whatever is at `path` - a link itself, never what it points to; nothing there is
/// fine.
fn remove_entry(path: &Path) -> Result<(), ConvertError> {
    match std::fs::symlink_metadata(path) {
        Ok(_) => std::fs::remove_file(path).map_err(|error| io(path, &error)),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(io(path, &error)),
    }
}

/// Makes `job`'s result with `ffmpeg` encoded `with`, reporting each new share done to `on_progress` and stopping when `cancel` changes; registers
/// it in the registry folder `dir` (none: not registered, the tests). Answers the names it
/// wrote: the result, and its poster when it made one.
///
/// # Errors
///
/// A stale `.part` that cannot go, ffmpeg's failure or the cancel ([`CANCELLED`]), a result
/// that appeared meanwhile, the rename or the registration.
pub async fn make(
    job: &Job,
    ffmpeg: &Path,
    with: EncodeWith,
    dir: Option<&Path>,
    cancel: &mut watch::Receiver<bool>,
    on_progress: &mut (dyn FnMut(u8) + Send),
) -> Result<Vec<String>, ConvertError> {
    let target = job.folder.join(&job.target);
    let part = job.folder.join(part_name(&job.target));
    // A part a run the app's end cut short left behind.
    remove_entry(&part)?;
    let args = convert_args(with.encoder, with.quality, with.threads, &job.source, &part);
    if let Err(error) = run_ffmpeg(ffmpeg, &args, cancel, on_progress).await {
        let _ = remove_entry(&part);
        return Err(ConvertError::Ffmpeg(error));
    }
    // Never over a file that appeared meanwhile; only the regular file ffmpeg wrote goes there.
    let written = std::fs::symlink_metadata(&part).map_err(|error| io(&part, &error))?;
    if std::fs::symlink_metadata(&target).is_ok() || !written.file_type().is_file() {
        let _ = remove_entry(&part);
        return Err(ConvertError::Exists(job.target.clone()));
    }
    std::fs::rename(&part, &target).map_err(|error| io(&target, &error))?;
    let mut names = vec![job.target.clone()];
    // The breakpoint's poster, where there is none yet (an MP4 from before posters).
    if let Some(poster) = poster_name(&job.target)
        && std::fs::symlink_metadata(job.folder.join(&poster)).is_err()
    {
        let posters =
            poster::write_all(ffmpeg, &job.folder, std::slice::from_ref(&job.target)).await;
        names.extend(posters);
    }
    if let Some(dir) = dir {
        let (dir, folder, registered) = (dir.to_path_buf(), job.folder.clone(), names.clone());
        tokio::task::spawn_blocking(move || {
            super::panel::register_saved(&dir, &folder, &registered)
        })
        .await
        .map_err(|error| ConvertError::Ffmpeg(error.to_string()))?
        .map_err(|error| io(&job.folder, &error))?;
    }
    Ok(names)
}

/// Runs `program` with `args` below the app's priority until it ends, `cancel` changes or it
/// stays silent for [`STALL_TIMEOUT`]; each new share done goes to `on_progress`.
async fn run_ffmpeg(
    program: &Path,
    args: &[OsString],
    cancel: &mut watch::Receiver<bool>,
    on_progress: &mut (dyn FnMut(u8) + Send),
) -> Result<(), String> {
    if *cancel.borrow() {
        return Err(CANCELLED.to_owned());
    }
    let mut child = rb_process::low_priority_command(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|error| format!("{} could not be started: {error}", program.display()))?;
    let (length, duration) = watch::channel(None::<f64>);
    let stderr = child.stderr.take();
    let errors = tokio::spawn(async move {
        let mut tail: VecDeque<String> = VecDeque::new();
        if let Some(stderr) = stderr {
            let mut lines = BufReader::new(stderr).lines();
            let mut known = false;
            while let Ok(Some(line)) = lines.next_line().await {
                if !known && let Some(seconds) = parse_duration(&line) {
                    known = true;
                    let _ = length.send(Some(seconds));
                }
                if tail.len() == ERROR_LINES {
                    tail.pop_front();
                }
                tail.push_back(line);
            }
        }
        Vec::from(tail).join(" | ")
    });
    let Some(stdout) = child.stdout.take() else {
        return Err("ffmpeg gave no output".to_owned());
    };
    let mut lines = BufReader::new(stdout).lines();
    let mut last = None;
    loop {
        tokio::select! {
            _ = cancel.changed() => {
                let _ = child.kill().await;
                return Err(CANCELLED.to_owned());
            }
            line = tokio::time::timeout(STALL_TIMEOUT, lines.next_line()) => match line {
                Err(_) => {
                    let _ = child.kill().await;
                    return Err(format!(
                        "ffmpeg made no progress for {} s and was stopped",
                        STALL_TIMEOUT.as_secs()
                    ));
                }
                Ok(Ok(Some(line))) => {
                    let now = match parse_progress(&line) {
                        Some(Progress::At(at)) => percent(at, *duration.borrow()),
                        Some(Progress::End) => Some(100),
                        None => None,
                    };
                    if let Some(now) = now
                        && last != Some(now)
                    {
                        last = Some(now);
                        on_progress(now);
                    }
                }
                Ok(_) => break,
            },
        }
    }
    let status = tokio::select! {
        _ = cancel.changed() => {
            let _ = child.kill().await;
            return Err(CANCELLED.to_owned());
        }
        status = child.wait() => {
            status.map_err(|error| format!("ffmpeg could not be waited for: {error}"))?
        }
    };
    let errors = errors.await.unwrap_or_default();
    if status.success() {
        Ok(())
    } else {
        Err(format!("ffmpeg ended with {status}: {errors}"))
    }
}

#[cfg(test)]
#[path = "convert_run_tests.rs"]
mod tests;
