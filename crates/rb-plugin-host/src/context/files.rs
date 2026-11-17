//! The disk side of `project.read`, `project.list`, `project-write.write` and `out.write`: every
//! path walked one segment
//! at a time from the project root, **no link followed** - a link anywhere on the way refuses the
//! path. A checked-in `.rbrowser/out` that links to `~/.ssh` must not turn a plugin's report into
//! a write there, even a first-party plugin's in an untrusted project.

use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use rand::Rng as _;
use rb_plugin_types::{Failure, FailureKind, codes, paths};
use tokio::io::AsyncWriteExt as _;

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

fn conflict(path: &str) -> Failure {
    Failure::coded(FailureKind::Other, codes::WRITE_CONFLICT)
        .with_param("path", path.chars().take(200).collect::<String>())
}

/// Replaces the project file at `segments` with `bytes` while it holds `expected` (`None`: while
/// nothing is there), in an existing folder: written to a new file beside it, flushed, then
/// renamed over it, so a reader sees the old file or the new one, never half of it, and a link
/// is never written through (a symbolic one refused, a hard one left holding the old bytes).
///
/// What changes between the comparison and the rename (another program's save in that instant)
/// is lost; the comparison guards the panel's minutes, not that instant. Plugins' writes run one
/// at a time, so two with the same `expected` never both pass; a read-only file is refused, and
/// the new file takes the old one's permissions (on Windows the read-only flag; its ACL is the
/// folder's afterwards, as with an editor's atomic save). The temporary file is removed on every
/// way out, a dropped call (the plugin's timeout) included (security review RB-110-13).
pub(super) async fn replace(
    root: &Path,
    segments: &[&str],
    shown: &str,
    bytes: &[u8],
    expected: Option<&[u8]>,
) -> Result<(), Failure> {
    let (file, folders) = segments.split_last().ok_or_else(|| not_found(shown))?;
    let folder = walk(root, folders, false, shown).await?;
    let path = folder.join(file);
    let _one_at_a_time = PROJECT_WRITES.lock().await;
    let (current, permissions) = match tokio::fs::symlink_metadata(&path).await {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            return Err(outside(shown));
        }
        Ok(metadata) if metadata.len() > rb_plugin_api::MAX_IO_BYTES as u64 => {
            return Err(too_large(rb_plugin_api::MAX_IO_BYTES));
        }
        Ok(metadata) if metadata.permissions().readonly() => return Err(read_only(shown)),
        Ok(metadata) => {
            may_write(&path, shown).await?;
            (
                Some(tokio::fs::read(&path).await.map_err(|error| io(&error))?),
                Some(metadata.permissions()),
            )
        }
        Err(error) if error.kind() == ErrorKind::NotFound => (None, None),
        Err(error) => return Err(io(&error)),
    };
    if current.as_deref() != expected {
        return Err(conflict(shown));
    }
    let mut random = [0u8; 8];
    rand::rng().fill_bytes(&mut random);
    let temporary = Temporary(Some(folder.join(temporary_name(&random))));
    let at = temporary.path();
    // `create_new`: never through anything planted at the name; only what it made is removed.
    let mut created = tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(at)
        .await
        .map_err(|error| io(&error))?;
    let written = match created.write_all(bytes).await {
        Ok(()) => created.sync_all().await,
        Err(error) => Err(error),
    };
    drop(created);
    written.map_err(|error| io(&error))?;
    if let Some(permissions) = permissions {
        tokio::fs::set_permissions(at, permissions)
            .await
            .map_err(|error| io(&error))?;
    }
    tokio::fs::rename(at, &path)
        .await
        .map_err(|error| io(&error))?;
    temporary.renamed();
    Ok(())
}

/// Whether the user may write the existing file at `path` itself: the rename needs only the
/// folder's permission, so another owner's `0644` file in a writable folder, or a file a Windows
/// ACL denies writing, would be replaced. Opened for writing, never truncated or created
/// (code review RB-110-13).
async fn may_write(path: &Path, shown: &str) -> Result<(), Failure> {
    match tokio::fs::OpenOptions::new().write(true).open(path).await {
        Ok(_) => Ok(()),
        Err(error) if error.kind() == ErrorKind::PermissionDenied => Err(read_only(shown)),
        Err(error) => Err(io(&error)),
    }
}

/// The temporary file's name beside the file it replaces: hidden, short - not the file's own
/// name, so a name near the file system's limit still fits - and named by no write glob.
fn temporary_name(random: &[u8; 8]) -> String {
    format!(".{}.rbrowser-tmp", rb_util::hex(random))
}

/// One project write at a time, across plugins: the comparison and the rename belong together.
static PROJECT_WRITES: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

fn read_only(path: &str) -> Failure {
    Failure::coded(FailureKind::AccessDenied, codes::PATH_NOT_WRITABLE)
        .with_param("path", path.chars().take(200).collect::<String>())
}

/// The temporary file of [`replace`], removed when dropped unless it was renamed into place.
struct Temporary(Option<PathBuf>);

impl Temporary {
    fn path(&self) -> &Path {
        self.0.as_deref().expect("set until renamed")
    }

    fn renamed(mut self) {
        self.0 = None;
    }
}

impl Drop for Temporary {
    fn drop(&mut self) {
        if let Some(path) = self.0.take() {
            // Blocking, but one small unlink on the way out of a failed or dropped write.
            let _ = std::fs::remove_file(path);
        }
    }
}

#[cfg(test)]
#[path = "files_tests.rs"]
mod tests;
