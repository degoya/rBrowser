//! Where a login state file lives: only under the project's `.rbrowser/out/` (git-ignored by
//! `rbrowser init`), named relative to it (`state/<session>.json` by default). A name that
//! climbs out, is absolute, names a drive or a share, or is no `.json` file is refused before
//! the disk is touched; a `.rbrowser` or `out` folder that is itself a link, and a file reached
//! through a link that leaves `out/`, are refused when it is read or written. Written files are the user's only (mode 0600 on Unix; on Windows the
//! project folder's ACL), through a temporary file renamed over the old one; reading stops at
//! [`MAX_FILE`].

use std::io::{self, Read as _};
use std::path::{Component, Path, PathBuf};

/// The folder every state file lives under, below the project root.
pub const OUT_DIR: [&str; 2] = [".rbrowser", "out"];
/// The longest name taken.
pub const MAX_NAME: usize = 200;
/// The largest file read, in bytes.
pub const MAX_FILE: usize = 8 * 1024 * 1024;

/// Why a name or file is refused.
#[derive(Debug, thiserror::Error)]
pub enum PathError {
    /// The name leaves `.rbrowser/out/` or is no `.json` file.
    #[error("the file must be a .json file under .rbrowser/out/, named relative to it")]
    Outside,
    /// Larger than [`MAX_FILE`].
    #[error("the file is larger than {MAX_FILE} bytes")]
    TooLarge,
    /// The disk.
    #[error("{0}")]
    Io(#[from] io::Error),
}

/// The default name of `session`'s state.
#[must_use]
pub fn default_name(session: &str) -> String {
    format!("state/{session}.json")
}

/// `<root>/.rbrowser/out`.
fn out_dir(root: &Path) -> PathBuf {
    OUT_DIR
        .iter()
        .fold(root.to_path_buf(), |path, part| path.join(part))
}

/// The file `name` (relative to `.rbrowser/out/`, `/` between folders) of the project at `root`.
///
/// # Errors
///
/// [`PathError::Outside`] for a name that leaves the folder or is no `.json` file.
pub fn resolve(root: &Path, name: &str) -> Result<PathBuf, PathError> {
    if name.len() > MAX_NAME || name.contains('\\') || name.contains(':') {
        return Err(PathError::Outside);
    }
    let relative = Path::new(name);
    let normal = relative
        .components()
        .all(|part| matches!(part, Component::Normal(_) | Component::CurDir));
    let json_file = relative
        .file_name()
        .and_then(|file| file.to_str())
        .is_some_and(|file| file.len() > ".json".len() && file.ends_with(".json"));
    if !normal || !json_file {
        return Err(PathError::Outside);
    }
    Ok(out_dir(root).join(relative))
}

/// Refuses a `.rbrowser` or `.rbrowser/out` that is itself a link or junction: a committed link
/// would carry the files wherever the repository points it (security review w28). An absent
/// folder is fine; [`write`] creates it.
fn unlinked_out(root: &Path) -> Result<(), PathError> {
    let rbrowser = root.join(OUT_DIR[0]);
    for folder in [rbrowser.join(OUT_DIR[1]), rbrowser] {
        match std::fs::symlink_metadata(&folder) {
            Ok(meta) if meta.file_type().is_symlink() => return Err(PathError::Outside),
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

/// Whether the real `dir` (links followed) lies in the real `.rbrowser/out/` of `root`, which is
/// no link itself.
fn inside_out(root: &Path, dir: &Path) -> Result<(), PathError> {
    unlinked_out(root)?;
    let out = std::fs::canonicalize(out_dir(root))?;
    let real = std::fs::canonicalize(dir)?;
    if real.starts_with(&out) {
        Ok(())
    } else {
        Err(PathError::Outside)
    }
}

/// The text of `path` (from [`resolve`]).
///
/// # Errors
///
/// [`PathError`]: a link out of the folder, too large, the disk.
pub fn read(root: &Path, path: &Path) -> Result<String, PathError> {
    inside_out(root, path)?;
    let file = std::fs::File::open(path)?;
    let mut text = String::new();
    let limit = u64::try_from(MAX_FILE)
        .unwrap_or(u64::MAX)
        .saturating_add(1);
    file.take(limit).read_to_string(&mut text)?;
    if text.len() > MAX_FILE {
        return Err(PathError::TooLarge);
    }
    Ok(text)
}

/// Writes `bytes` to `path` (from [`resolve`]), the user's only, replacing the file.
///
/// # Errors
///
/// [`PathError`]: a link out of the folder, the disk.
pub fn write(root: &Path, path: &Path, bytes: &[u8]) -> Result<(), PathError> {
    let dir = path.parent().ok_or(PathError::Outside)?;
    unlinked_out(root)?;
    std::fs::create_dir_all(dir)?;
    inside_out(root, dir)?;
    rb_util::fs::write_atomic_private(path, bytes).map_err(PathError::Io)
}

#[cfg(test)]
#[path = "paths_tests.rs"]
mod tests;
