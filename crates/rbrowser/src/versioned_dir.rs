//! A folder of versions: `<dir>/<version>/` beside `<dir>/current.json` naming the active one,
//! as the Web Store extensions (`extensions::store`, RB-080-23) and the managed tools
//! (`tools::store`) keep them - one activation and one prune for both (RB-094-10).
//!
//! [`activate`] moves an unpacked folder into place and points `dir` at it, each step undone
//! when a later one fails: a folder of that version already there (not the active one) is
//! renamed aside first (`.retired-<version>-<tag>`), never deleted before the new one is in
//! place, and renamed back on a failure; the pointer is replaced whole
//! ([`rb_util::fs::write_atomic`]), so the pointer before stays until then. A folder set aside
//! that cannot be put back is logged as an error and left where it is.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// The pointer file in a folder of versions.
pub const POINTER: &str = "current.json";

/// The pointer file's content.
#[derive(Debug, Deserialize, Serialize)]
struct Pointer {
    version: String,
}

/// The version `dir`'s pointer names, unchecked; `None` without a pointer that reads.
pub fn read_pointer(dir: &Path) -> Option<String> {
    let text = fs::read_to_string(dir.join(POINTER)).ok()?;
    serde_json::from_str::<Pointer>(&text)
        .ok()
        .map(|pointer| pointer.version)
}

/// Moves `unpacked` to `<dir>/<version>/` and points `dir` at it; `tag` makes the name of a
/// folder set aside its own (an attempt, a process). The folder in place comes back.
///
/// # Errors
///
/// A rename or the pointer fails; what was there before is put back.
pub fn activate(dir: &Path, version: &str, unpacked: &Path, tag: &str) -> io::Result<PathBuf> {
    let target = dir.join(version);
    let aside = dir.join(format!(".retired-{version}-{tag}"));
    let retired = if target.exists() {
        fs::rename(&target, &aside)?;
        true
    } else {
        false
    };
    let restore = || {
        if retired && let Err(error) = fs::rename(&aside, &target) {
            tracing::error!(
                %error,
                "the version set aside could not be put back; it stays at {}",
                aside.display()
            );
        }
    };
    if let Err(error) = fs::rename(unpacked, &target) {
        restore();
        return Err(error);
    }
    if let Err(error) = write_pointer(dir, version) {
        // The pointer before still names the version before: undo the new folder.
        if let Err(removal) = fs::remove_dir_all(&target) {
            tracing::warn!(%removal, "the new version stays: {}", target.display());
        }
        restore();
        return Err(error);
    }
    Ok(target)
}

/// Removes every folder in `dir` but `keep` (the versions before, the folders set aside); what
/// cannot go (a running program) stays.
pub fn prune(dir: &Path, keep: &str) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir()
            && entry.file_name() != keep
            && let Err(error) = fs::remove_dir_all(&path)
        {
            tracing::warn!(%error, "an old version stays: {}", path.display());
        }
    }
}

/// Points `dir` at `version`, the pointer replaced whole.
fn write_pointer(dir: &Path, version: &str) -> io::Result<()> {
    let text = serde_json::to_string(&Pointer {
        version: version.to_owned(),
    })
    .map_err(io::Error::other)?;
    rb_util::fs::write_atomic(&dir.join(POINTER), text.as_bytes())
}

#[cfg(test)]
#[path = "versioned_dir_tests.rs"]
mod tests;
