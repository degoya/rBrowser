//! Where an export is written. An agent's path (the MCP tool `checks_export`) and the command's
//! stay below the project's `.rbrowser/out/` or the person's own export folder: a relative path
//! lands in the export folder (`checks.export.dir`, else `.rbrowser/out/exports`), an absolute
//! one must lie inside `.rbrowser/out/` or the folder the user settings name; `..`, `.`, a
//! drive or stream name in a relative path, a device or share path and control characters are
//! refused before anything is touched, and on writing a folder link on the way (from the
//! project's root, or below the user's folder) is refused too. `rbrowser check --out` is the
//! person's own path: any plain absolute file ([`write_file`]).

use std::io;
use std::path::{Component, Path, PathBuf};

/// The project's output folder below its root.
pub const OUT_DIR: [&str; 2] = [".rbrowser", "out"];
/// The export folder below [`OUT_DIR`] when `checks.export.dir` is not set.
pub const EXPORTS_DIR: &str = "exports";
/// The longest path taken.
pub const MAX_PATH_LEN: usize = 1024;

/// Why a path was refused.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum PathError {
    /// No project is open and the user settings name no absolute export folder.
    #[error("no project is open and no export folder is set")]
    NoBase,
    /// The path or a folder setting is not allowed.
    #[error("{0}")]
    Refused(String),
}

/// Where exports may go.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bases {
    /// The export folder: a default file name and a relative path land here.
    pub folder: PathBuf,
    /// The folders an absolute path may lie in: `.rbrowser/out/` and the user's own folder.
    pub allowed: Vec<PathBuf>,
    /// The project's root: `.rbrowser/out/` must really lie in it (no link on the way, which a
    /// checked-in project could carry).
    pub root: Option<PathBuf>,
}

/// `.rbrowser/out` below `root`.
fn out_of(root: &Path) -> PathBuf {
    OUT_DIR
        .iter()
        .fold(root.to_path_buf(), |path, part| path.join(part))
}

fn refused(text: &str, why: &str) -> PathError {
    PathError::Refused(format!(
        "{}: {why}",
        text.chars().take(80).collect::<String>()
    ))
}

/// The parts of the relative path `text`: no leading separator, no `:` (a drive or a stream
/// name), no `..`, no control character, at most [`MAX_PATH_LEN`] bytes, at least one part.
fn relative_parts(text: &str) -> Result<Vec<&str>, PathError> {
    plain(text)?;
    if text.starts_with(['/', '\\']) || text.contains(':') || Path::new(text).is_absolute() {
        return Err(refused(text, "not a relative path"));
    }
    let mut parts = Vec::new();
    for part in text.split(['/', '\\']) {
        match part {
            "" | "." => {}
            ".." => return Err(refused(text, "a path may not go up with ..")),
            part => parts.push(part),
        }
    }
    if parts.is_empty() {
        return Err(refused(text, "no file name"));
    }
    Ok(parts)
}

/// `text` is 1 to [`MAX_PATH_LEN`] bytes without control characters.
fn plain(text: &str) -> Result<(), PathError> {
    if text.is_empty() || text.len() > MAX_PATH_LEN || text.chars().any(char::is_control) {
        return Err(refused(
            text,
            &format!("a path must have 1 to {MAX_PATH_LEN} plain characters"),
        ));
    }
    Ok(())
}

/// The absolute folder or file `text`: plain, without `.` or `..`, no device or verbatim prefix
/// and no network share (two leading separators).
fn absolute(text: &str) -> Result<PathBuf, PathError> {
    plain(text)?;
    let mut chars = text.chars();
    if matches!(
        (chars.next(), chars.next()),
        (Some('/' | '\\'), Some('/' | '\\'))
    ) {
        return Err(refused(text, "a network share or device is not allowed"));
    }
    crate::instance::protocol::check_out_dir(text).map_err(PathError::Refused)
}

/// Where exports may go for the project at `root` (`None`: none open), with `checks.export.dir`
/// of the project (`project_dir`, relative only, below `.rbrowser/out/`) and of the user
/// settings (`user_dir`, relative below `.rbrowser/out/` or absolute); the project's wins as
/// the folder, the user's absolute folder stays allowed.
///
/// # Errors
///
/// A folder setting that is not allowed, or nowhere to write ([`PathError::NoBase`]).
pub fn bases(
    root: Option<&Path>,
    project_dir: Option<&str>,
    user_dir: Option<&str>,
) -> Result<Bases, PathError> {
    let out = root.map(out_of);
    let below_out = |text: &str| -> Result<PathBuf, PathError> {
        let parts = relative_parts(text)?;
        let out = out.clone().ok_or(PathError::NoBase)?;
        Ok(parts.iter().fold(out, |path, part| path.join(part)))
    };
    let project_dir = project_dir.map(str::trim).filter(|text| !text.is_empty());
    let user_dir = user_dir.map(str::trim).filter(|text| !text.is_empty());
    let user_absolute = match user_dir {
        Some(text) if Path::new(text).is_absolute() || text.starts_with(['/', '\\']) => {
            Some(absolute(text)?)
        }
        _ => None,
    };
    let folder = if let Some(text) = project_dir {
        below_out(text)?
    } else if let Some(dir) = &user_absolute {
        dir.clone()
    } else if let Some(text) = user_dir {
        below_out(text)?
    } else {
        out.clone().ok_or(PathError::NoBase)?.join(EXPORTS_DIR)
    };
    let allowed = out.into_iter().chain(user_absolute).collect();
    Ok(Bases {
        folder,
        allowed,
        root: root.map(Path::to_path_buf),
    })
}

/// The file an export goes to: `default_name` in the export folder when `requested` is `None`;
/// a relative path below the export folder; an absolute path inside an allowed folder.
///
/// # Errors
///
/// The path is refused (see the module's account).
pub fn resolve(
    bases: &Bases,
    requested: Option<&str>,
    default_name: &str,
) -> Result<PathBuf, PathError> {
    let Some(text) = requested else {
        return Ok(bases.folder.join(default_name));
    };
    plain(text)?;
    if Path::new(text).is_absolute() || text.starts_with(['/', '\\']) {
        let path = absolute(text)?;
        let inside = bases
            .allowed
            .iter()
            .any(|base| path.starts_with(base) && path != *base);
        if !inside {
            return Err(refused(
                text,
                "an export is written only below .rbrowser/out/ or the export folder of the user settings",
            ));
        }
        return Ok(path);
    }
    let parts = relative_parts(text)?;
    Ok(parts
        .iter()
        .fold(bases.folder.clone(), |path, part| path.join(part)))
}

/// Writes `text` to `path` (from [`resolve`]), making its folders one by one from the project's
/// root (for `.rbrowser/out/`) or the user's folder - refused when a folder on the way is a link
/// (a checked-in project could carry one that leads elsewhere), when `path` lies in no allowed
/// folder, or when it is a link itself. Nothing is made outside the allowed folders.
///
/// # Errors
///
/// The path is refused (`PermissionDenied`), a folder could not be made, or the write failed.
pub fn write_inside(bases: &Bases, path: &Path, text: &str) -> io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "the path has no folder"))?;
    let base = bases
        .allowed
        .iter()
        .find(|base| parent.starts_with(base))
        .ok_or_else(|| denied("the path lies outside the folders an export may be written to"))?;
    let anchor = match bases.root.as_deref() {
        Some(root) if *base == out_of(root) => root.to_path_buf(),
        // The user's own folder: made as named.
        _ => {
            std::fs::create_dir_all(base)?;
            base.clone()
        }
    };
    make_folders(&anchor, parent)?;
    refuse_link(path)?;
    std::fs::write(path, text)
}

fn denied(why: &str) -> io::Error {
    io::Error::new(io::ErrorKind::PermissionDenied, why.to_owned())
}

/// Makes the folders of `folder` below `anchor` one by one; a link among them is refused.
fn make_folders(anchor: &Path, folder: &Path) -> io::Result<()> {
    let rest = folder
        .strip_prefix(anchor)
        .map_err(|_| denied("the folder lies outside its anchor"))?;
    let mut current = anchor.to_path_buf();
    for part in rest.components() {
        let Component::Normal(name) = part else {
            return Err(denied("the folder is not plain"));
        };
        current.push(name);
        match std::fs::symlink_metadata(&current) {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(denied(
                    "a folder link leads out of the folders an export may be written to",
                ));
            }
            Ok(meta) if meta.is_dir() => {}
            Ok(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    "a file stands where a folder belongs",
                ));
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                std::fs::create_dir(&current)?;
            }
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

/// Writes `text` to the person's own file `path` (`rbrowser check --out`), making its folders.
///
/// # Errors
///
/// The folder could not be made, `path` is a link, or the write failed.
pub fn write_file(path: &Path, text: &str) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    refuse_link(path)?;
    std::fs::write(path, text)
}

fn refuse_link(path: &Path) -> io::Result<()> {
    match std::fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_symlink() => Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "the export file is a link",
        )),
        _ => Ok(()),
    }
}

#[cfg(test)]
#[path = "target_tests.rs"]
mod tests;
