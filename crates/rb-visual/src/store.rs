//! Writing a screenshot: below the output folder, at the relative path the pattern gave, with
//! `.png` appended. The folders below the output folder are real ones, never links. An existing file is never overwritten; the next free `-2`, `-3`, ... is
//! taken instead (two views with the same breakpoint in one run).

use std::fs::OpenOptions;
use std::io::Write as _;
use std::path::{Component, Path, PathBuf};

use crate::VisualError;

/// The most numbered names tried for one file.
const MAX_SUFFIX: u32 = 999;

/// Writes `png` to `dir/<relative>.png` (or the first free `-<n>` beside it), creating the
/// folders; returns the file written. `relative` must be a plain relative path.
pub fn save_png(dir: &Path, relative: &Path, png: &[u8]) -> Result<PathBuf, VisualError> {
    let plain = !relative.as_os_str().is_empty()
        && relative
            .components()
            .all(|part| matches!(part, Component::Normal(_)));
    let Some(stem) = relative.file_name().and_then(|name| name.to_str()) else {
        return Err(VisualError::Pattern(format!(
            "{} names no file",
            relative.display()
        )));
    };
    if !plain {
        return Err(VisualError::Pattern(format!(
            "{} leaves the screenshot folder",
            relative.display()
        )));
    }
    std::fs::create_dir_all(dir).map_err(|source| io(dir, source))?;
    let parent = real_subfolders(dir, relative.parent().unwrap_or(Path::new("")))?;
    for n in 1..=MAX_SUFFIX {
        let name = if n == 1 {
            format!("{stem}.png")
        } else {
            format!("{stem}-{n}.png")
        };
        let path = parent.join(name);
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut file) => {
                file.write_all(png).map_err(|source| io(&path, source))?;
                return Ok(path);
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(source) => return Err(io(&path, source)),
        }
    }
    Err(VisualError::Folder(format!(
        "{} has {MAX_SUFFIX} files of that name",
        parent.display()
    )))
}

/// `dir` joined with `folders` (plain parts), each a real folder: created when missing, never a
/// link (a link checked into the project may lead out of it) and never a file.
fn real_subfolders(dir: &Path, folders: &Path) -> Result<PathBuf, VisualError> {
    let mut path = dir.to_path_buf();
    for part in folders.components() {
        path.push(part);
        match std::fs::symlink_metadata(&path) {
            Ok(meta) if meta.is_dir() => {}
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(VisualError::Folder(format!(
                    "{} is a link, which may leave the screenshot folder",
                    path.display()
                )));
            }
            Ok(_) => {
                return Err(VisualError::Folder(format!(
                    "{} is not a folder",
                    path.display()
                )));
            }
            Err(_) => match std::fs::create_dir(&path) {
                Ok(()) => {}
                // Made by a run at the same time: it must still be a real folder.
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    if !std::fs::symlink_metadata(&path).is_ok_and(|meta| meta.is_dir()) {
                        return Err(VisualError::Folder(format!(
                            "{} is not a folder",
                            path.display()
                        )));
                    }
                }
                Err(source) => return Err(io(&path, source)),
            },
        }
    }
    Ok(path)
}

fn io(path: &Path, source: std::io::Error) -> VisualError {
    VisualError::Io {
        path: path.display().to_string(),
        source,
    }
}

#[cfg(test)]
#[path = "store_tests.rs"]
mod tests;
