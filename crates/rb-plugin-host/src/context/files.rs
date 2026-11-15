//! The disk side of `project.read`, `project.list` and `out.write`: every path walked one segment
//! at a time from the project root, **no link followed** - a link anywhere on the way refuses the
//! path. A checked-in `.rbrowser/out` that links to `~/.ssh` must not turn a plugin's report into
//! a write there, even a first-party plugin's in an untrusted project.

use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use rb_plugin_types::{Failure, FailureKind, codes, paths};

use crate::failure::too_large;

fn outside(path: &str) -> Failure {
    paths::PathRefusal::Parent.failure(path)
}

fn not_found(path: &str) -> Failure {
    Failure::coded(FailureKind::NotFound, codes::NOT_FOUND)
        .with_param("path", path.chars().take(200).collect::<String>())
}

fn io(error: &std::io::Error) -> Failure {
    tracing::debug!(%error, "a plugin's file operation failed");
    Failure::new(FailureKind::Io)
}

/// What is at `path`, without following a link: `Ok(None)` when nothing is.
async fn kind(path: &Path) -> Result<Option<std::fs::FileType>, std::io::Error> {
    match tokio::fs::symlink_metadata(path).await {
        Ok(metadata) => Ok(Some(metadata.file_type())),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

/// `root` joined with `folders`, each an existing real folder (created when `create`), none a
/// link.
async fn walk(
    root: &Path,
    folders: &[&str],
    create: bool,
    shown: &str,
) -> Result<PathBuf, Failure> {
    let mut current = root.to_path_buf();
    for folder in folders {
        current.push(folder);
        match kind(&current).await.map_err(|error| io(&error))? {
            Some(found) if found.is_symlink() => return Err(outside(shown)),
            Some(found) if found.is_dir() => {}
            Some(_) => return Err(not_found(shown)),
            None if create => match tokio::fs::create_dir(&current).await {
                Ok(()) => {}
                Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                    // Created by a racing call: it must still be a real folder.
                    let found = kind(&current).await.map_err(|error| io(&error))?;
                    if !found.is_some_and(|found| found.is_dir() && !found.is_symlink()) {
                        return Err(outside(shown));
                    }
                }
                Err(error) => return Err(io(&error)),
            },
            None => return Err(not_found(shown)),
        }
    }
    Ok(current)
}

/// Reads the project file at `segments`.
pub(super) async fn read(root: &Path, segments: &[&str], shown: &str) -> Result<Vec<u8>, Failure> {
    let (file, folders) = segments.split_last().ok_or_else(|| not_found(shown))?;
    let folder = walk(root, folders, false, shown).await?;
    let path = folder.join(file);
    let metadata = match tokio::fs::symlink_metadata(&path).await {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == ErrorKind::NotFound => return Err(not_found(shown)),
        Err(error) => return Err(io(&error)),
    };
    if metadata.file_type().is_symlink() {
        return Err(outside(shown));
    }
    if !metadata.is_file() {
        return Err(not_found(shown));
    }
    if metadata.len() > rb_plugin_api::MAX_IO_BYTES as u64 {
        return Err(too_large(rb_plugin_api::MAX_IO_BYTES));
    }
    tokio::fs::read(&path).await.map_err(|error| io(&error))
}

/// The entries of the project folder at `segments`: `(name, is_folder)`, links left out.
pub(crate) async fn list(
    root: &Path,
    segments: &[&str],
    shown: &str,
) -> Result<Vec<(String, bool)>, Failure> {
    let folder = walk(root, segments, false, shown).await?;
    let mut reader = tokio::fs::read_dir(&folder)
        .await
        .map_err(|error| io(&error))?;
    let mut entries = Vec::new();
    while let Some(entry) = reader.next_entry().await.map_err(|error| io(&error))? {
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
        let Ok(found) = entry.file_type().await else {
            continue;
        };
        if found.is_symlink() || paths::relative_segments(&name).is_err() {
            continue;
        }
        entries.push((name, found.is_dir()));
    }
    entries.sort();
    Ok(entries)
}

/// Writes `bytes` at `segments` (which start with `.rbrowser/out/<plugin-id>`), creating the
/// folders on the way.
pub(crate) async fn write(
    root: &Path,
    segments: &[&str],
    shown: &str,
    bytes: &[u8],
) -> Result<(), Failure> {
    let (file, folders) = segments.split_last().ok_or_else(|| not_found(shown))?;
    let folder = walk(root, folders, true, shown).await?;
    let path = folder.join(file);
    if let Some(found) = kind(&path).await.map_err(|error| io(&error))?
        && (found.is_symlink() || found.is_dir())
    {
        return Err(outside(shown));
    }
    tokio::fs::write(&path, bytes)
        .await
        .map_err(|error| io(&error))
}
