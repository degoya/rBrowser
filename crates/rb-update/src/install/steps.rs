//! The steps around the switch: the artifact checked once more and unpacked from the handle that
//! was hashed, and the leftovers of a finished update.
//!
//! Copied and adapted from rCommander's `crates/rc-update/src/install/steps.rs` (RB-090-02):
//! no database copy.

use std::fs;
use std::io::{Read as _, Seek as _, SeekFrom};
use std::path::Path;

use sha2::{Digest, Sha256};

use super::{
    InstallError, Journal, Phase, Plan, durable::remove_any, process::UPDATER_DIR, trash,
    update_dir,
};
use crate::sign::hex;

/// Hashes the downloaded artifact again: it lay in the data directory between the check and
/// this use, and only what the signed manifest describes is ever installed.
///
/// # Errors
///
/// `update.digest_mismatch` when size or SHA-256 differ, `update.download_failed` when it is gone.
pub fn verify_artifact(plan: &Plan) -> Result<(), InstallError> {
    open_artifact(plan).map(drop)
}

/// The plan's artifact, opened and checked: the handle the switch unpacks from.
///
/// # Errors
///
/// As [`verify_artifact`].
pub fn open_artifact(plan: &Plan) -> Result<fs::File, InstallError> {
    open_checked(&plan.artifact, &plan.sha256, plan.size)
}

/// Opens `path`, checks its size and SHA-256 and returns the handle at the file's start: what is
/// read through it is what was hashed, where hashing one opening and using the path again would
/// let a file swapped in between be installed. On Windows the handle shares reading only, so
/// while it is open nobody writes, renames or removes the file.
///
/// # Errors
///
/// `update.digest_mismatch` or `update.download_failed`.
pub(crate) fn open_checked(path: &Path, sha256: &str, size: u64) -> Result<fs::File, InstallError> {
    let gone = |error: std::io::Error| {
        InstallError::new(
            "update.download_failed",
            format!("{}: {error}", path.display()),
        )
    };
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt as _;
        const FILE_SHARE_READ: u32 = 0x0000_0001;
        options.share_mode(FILE_SHARE_READ);
    }
    let mut file = options.open(path).map_err(gone)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; 1 << 16];
    let mut read_bytes: u64 = 0;
    loop {
        let read = file.read(&mut buffer).map_err(gone)?;
        if read == 0 {
            break;
        }
        read_bytes = read_bytes.saturating_add(u64::try_from(read).unwrap_or(u64::MAX));
        hasher.update(&buffer[..read]);
    }
    if size != read_bytes || hex(&hasher.finalize()) != sha256 {
        return Err(InstallError::new(
            "update.digest_mismatch",
            format!("{} is not the file that was checked", path.display()),
        ));
    }
    file.seek(SeekFrom::Start(0)).map_err(gone)?;
    Ok(file)
}

/// Removes what a finished update leaves beside the program - the staging, the files of a
/// rolled-back version and, after a proven update, `.previous/` - through the trash, and the
/// updater's copy in the data folder. The downloads stay until the next update.
pub fn clean_up(journal: &Journal) {
    let install = &journal.plan.install_dir;
    let mut beside_program = vec![journal.staged_dir(), journal.failed_dir()];
    if journal.phase == Phase::Verified {
        beside_program.push(journal.previous_dir());
    }
    for leftover in beside_program {
        if let Err(error) = trash::discard(&leftover, install) {
            tracing::warn!(
                error = format!("{error:#}"),
                "an update leftover stays until a later start"
            );
        }
    }
    let updater = update_dir(&journal.plan.data_dir).join(UPDATER_DIR);
    if let Err(error) = remove_any(&updater) {
        tracing::warn!(%error, "an update leftover stays until a later start");
    }
}
