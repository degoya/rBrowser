//! The verified download: an artifact streamed to disk and kept only if it is exactly the file the
//! signed manifest describes. The install uses what this returns and nothing else.
//!
//! The bytes go to `<name>.partial` beside the target and are hashed on the way; the size is held
//! to the manifest's at every chunk, so a server that sends more is cut off (the size cap). Only a
//! file whose size and SHA-256 both match is renamed to its final name; anything else is
//! deleted. Before a byte is fetched the free space is checked: the whole program folder is
//! replaced (CEF moves with every release, no delta), so the archive and its unpacked copy must
//! both fit.
//!
//! Copied and adapted from rCommander's `crates/rc-update/src/download.rs` (RB-090-02): blocking.

use std::fs;
use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use sha2::{Digest, Sha256};

use crate::{Fetcher, error::UpdateError, manifest::Artifact, sign::hex};

/// The download's progress and its cancel switch, shared with whoever shows or stops it.
#[derive(Clone, Debug, Default)]
pub struct Progress {
    received: Arc<AtomicU64>,
    cancelled: Arc<AtomicBool>,
}

impl Progress {
    /// Bytes written so far.
    #[must_use]
    pub fn received(&self) -> u64 {
        self.received.load(Ordering::Relaxed)
    }

    /// Stops the download at its next chunk; it leaves no file.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }

    fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }
}

/// Refuses a download without twice the artifact's size free (the archive and its unpacked
/// copy); `free` is the volume's, `None` when it does not say.
///
/// # Errors
///
/// `update.no_space`.
pub fn check_space(artifact: &Artifact, free: Option<u64>) -> Result<(), UpdateError> {
    let required = artifact.size.saturating_mul(2);
    match free {
        Some(free) if free < required => Err(UpdateError::NoSpace { free, required }),
        _ => Ok(()),
    }
}

/// Downloads `artifact` into `directory` and returns the verified file's path.
///
/// # Errors
///
/// `update.invalid` for an address that is not https, `update.download_failed`,
/// `update.size_mismatch`, `update.digest_mismatch`, or `update.failed` for the disk (and for a
/// cancel); no file stays under the final name then.
pub fn download_verified(
    fetcher: &dyn Fetcher,
    artifact: &Artifact,
    directory: &Path,
    progress: &Progress,
) -> Result<PathBuf, UpdateError> {
    let url = https_url(artifact)?;
    let target = directory.join(file_name(&url));
    let partial = directory.join(format!("{}.partial", file_name(&url)));
    fs::create_dir_all(directory).map_err(|error| UpdateError::Other(error.into()))?;
    match stream_to(fetcher, &url, artifact, &partial, progress) {
        Ok(()) => {
            fs::rename(&partial, &target).map_err(|error| UpdateError::Other(error.into()))?;
            Ok(target)
        }
        Err(error) => {
            let _ = fs::remove_file(&partial);
            Err(error)
        }
    }
}

/// The verified download of `artifact` in `directory`, when one is there already - another
/// window's, or an earlier run's: its size and SHA-256 are checked again before it is taken.
#[must_use]
pub fn already_downloaded(artifact: &Artifact, directory: &Path) -> Option<PathBuf> {
    let url = https_url(artifact).ok()?;
    let target = directory.join(file_name(&url));
    crate::install::steps::open_checked(&target, &artifact.sha256, artifact.size)
        .ok()
        .map(|_| target)
}

fn https_url(artifact: &Artifact) -> Result<url::Url, UpdateError> {
    let url = url::Url::parse(&artifact.url)
        .map_err(|error| UpdateError::Invalid(format!("artifact URL: {error}")))?;
    if url.scheme() != "https" {
        return Err(UpdateError::Invalid(format!("{url} is not https")));
    }
    Ok(url)
}

fn stream_to(
    fetcher: &dyn Fetcher,
    url: &url::Url,
    artifact: &Artifact,
    partial: &Path,
    progress: &Progress,
) -> Result<(), UpdateError> {
    let mut download = fetcher
        .open(url)
        .map_err(|error| UpdateError::Download(format!("{error:#}")))?;
    if let Some(length) = download.length
        && length != artifact.size
    {
        return Err(UpdateError::SizeMismatch {
            expected: artifact.size,
            saw: length,
        });
    }
    let mut file = fs::File::create(partial).map_err(|error| UpdateError::Other(error.into()))?;
    let mut hasher = Sha256::new();
    let mut written: u64 = 0;
    let mut buffer = vec![0_u8; 64 * 1024];
    loop {
        if progress.is_cancelled() {
            return Err(UpdateError::Other(anyhow::anyhow!(
                "the download was cancelled"
            )));
        }
        let read = download
            .body
            .read(&mut buffer)
            .map_err(|error| UpdateError::Download(error.to_string()))?;
        if read == 0 {
            break;
        }
        written = written.saturating_add(u64::try_from(read).unwrap_or(u64::MAX));
        if written > artifact.size {
            return Err(UpdateError::SizeMismatch {
                expected: artifact.size,
                saw: written,
            });
        }
        hasher.update(&buffer[..read]);
        file.write_all(&buffer[..read])
            .map_err(|error| UpdateError::Other(error.into()))?;
        progress.received.store(written, Ordering::Relaxed);
    }
    file.sync_all()
        .map_err(|error| UpdateError::Other(error.into()))?;
    drop(file);
    if written != artifact.size {
        return Err(UpdateError::SizeMismatch {
            expected: artifact.size,
            saw: written,
        });
    }
    if hex(&hasher.finalize()) != artifact.sha256 {
        return Err(UpdateError::DigestMismatch);
    }
    Ok(())
}

/// The URL's last segment, reduced to characters that are safe in a file name everywhere.
pub(crate) fn file_name(url: &url::Url) -> String {
    let last = url
        .path_segments()
        .and_then(|mut segments| segments.next_back())
        .unwrap_or_default();
    let cleaned: String = last
        .chars()
        .filter(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_')
        })
        .collect();
    let cleaned = cleaned.trim_start_matches('.');
    if cleaned.is_empty() {
        "rbrowser-update.bin".to_owned()
    } else {
        cleaned.to_owned()
    }
}

#[cfg(test)]
#[path = "download_tests.rs"]
mod tests;
