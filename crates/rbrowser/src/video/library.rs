//! The recordings of the open project for the Videos panel (RB-110-12): what lies in
//! `.rbrowser/out/video/<recording>/`, one file per view, and the panel's deletions.
//!
//! **Only what rBrowser wrote** (security review): a recording is listed, served and deleted
//! only when the registry outside the project knows it ([`super::registry`]), and a file only
//! while its size and time are the ones the recorder left - a video a repository checked in is
//! never handed to the shell's renderer. A recording id is the recorder's own form
//! (`2026-11-16_14-03-07`), a file a plain name of the recorder's characters
//! (`rb_visual::naming::sanitize_segment`) with the extension `mp4` or `webm`, held to the plugin
//! path rules too (`rb_plugin_types::paths`: no device name, no short name, no trailing dot).
//!
//! Every path is walked one folder at a time from the project root and **no link is followed**
//! ([`RunStore`], the sitemap capture's walk): a checked-in `.rbrowser/out` that links elsewhere
//! lists nothing and serves nothing. A file is opened without following a link in its last
//! component and its type, identity and stamp are taken from the open handle
//! (`crate::analyzer::project_files::no_follow`); a deletion checks the path again right before
//! it removes the file. **Residual window:** a local process that can write the project can swap
//! a folder on the way for a link between the walk and the open (only the last component is
//! opened without following) or a file between the last check and `remove_file` - std offers no
//! delete by handle; the identity check on the handle and the registry stamp keep a swapped file
//! from being served.

use std::fs::{File, Metadata};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use rb_visual::VisualError;
use rb_visual::sitemap_store::{RunStore, run_ids_in};

use super::poster::{self, poster_name};
use super::registry::Recorded;
use super::settings::OUT_DIR;
use crate::analyzer::project_files::no_follow;

/// The extensions of the recorder's files.
pub const EXTENSIONS: [&str; 2] = ["mp4", "webm"];
/// The longest file name, in bytes.
const MAX_NAME: usize = 255;

/// What a deletion removed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Deleted {
    /// The files, by name.
    pub names: Vec<String>,
    /// The recording's folder went too.
    pub folder_removed: bool,
}

/// Why a recording or file was not listed, served or deleted.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum LibraryError {
    /// Not a plain name, not a video file, or a link on the way.
    #[error("`{0}` is no recording or video file of the project")]
    Refused(String),
    /// Nothing there.
    #[error("there is no such recording or video file")]
    NotFound,
    /// The recording is being written.
    #[error("the recording is still running; stop it first")]
    Running,
    /// The disk.
    #[error("{0}")]
    Io(String),
}

impl LibraryError {
    /// The stable code of an MCP or panel failure.
    pub fn code(&self) -> &'static str {
        match self {
            Self::Refused(_) => "video.refused",
            Self::NotFound => "video.not_found",
            Self::Running => "video.busy",
            Self::Io(_) => "video.failed",
        }
    }
}

impl From<VisualError> for LibraryError {
    fn from(error: VisualError) -> Self {
        match error {
            VisualError::Outside(name) => Self::Refused(name),
            other => Self::Io(other.to_string()),
        }
    }
}

/// One video file of a recording.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VideoEntry {
    /// `mobile.mp4`.
    pub name: String,
    /// Its size.
    pub bytes: u64,
    /// When it was last written.
    pub modified: Option<SystemTime>,
    /// Its poster's name (`mobile.jpg`) while the poster is the one rBrowser wrote.
    pub poster: Option<String>,
}

/// One recording: its folder and its video files, by name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordingEntry {
    /// The folder name, `2026-11-16_14-03-07`.
    pub id: String,
    /// The folder.
    pub dir: PathBuf,
    /// Its video files, by name.
    pub files: Vec<VideoEntry>,
}

impl RecordingEntry {
    /// When its newest file was written; `None` without a dated file.
    #[must_use]
    pub fn modified(&self) -> Option<SystemTime> {
        self.files.iter().filter_map(|file| file.modified).max()
    }
}

/// The characters the recorder's names are made of.
pub(super) fn is_plain(name: &str) -> bool {
    (1..=MAX_NAME).contains(&name.len())
        && !name.starts_with('.')
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '@' | '+'))
        && rb_plugin_types::paths::relative_segments(name).is_ok_and(|segments| segments.len() == 1)
}

/// Whether `id` is a recording folder's name as the recorder writes it
/// (`rb_visual::Stamp::date_time`): `YYYY-MM-DD_HH-MM-SS`, digits where digits go.
#[must_use]
pub fn is_recording_id(id: &str) -> bool {
    const SHAPE: &[u8; 19] = b"0000-00-00_00-00-00";
    id.len() == SHAPE.len()
        && id
            .bytes()
            .zip(SHAPE.iter())
            .all(|(byte, shape)| match shape {
                b'0' => byte.is_ascii_digit(),
                other => byte == *other,
            })
}

/// The lower-case extension of `name` when it is a video's.
fn extension(name: &str) -> Option<&'static str> {
    let (stem, extension) = name.rsplit_once('.')?;
    if stem.is_empty() {
        return None;
    }
    EXTENSIONS
        .into_iter()
        .find(|known| known.eq_ignore_ascii_case(extension))
}

/// Whether `name` may name a recording's video file.
#[must_use]
pub fn is_video_name(name: &str) -> bool {
    is_plain(name) && extension(name).is_some()
}

/// Whether the video route serves a file of this name: a video or a video's poster
/// ([`poster`]).
#[must_use]
pub fn is_served_name(name: &str) -> bool {
    is_video_name(name) || poster::is_poster_name(name)
}

/// The MIME type of a video file or a poster.
#[must_use]
pub fn mime(name: &str) -> Option<&'static str> {
    if poster::is_poster_name(name) {
        return Some(poster::MIME);
    }
    match extension(name)? {
        "mp4" => Some("video/mp4"),
        _ => Some("video/webm"),
    }
}

/// The breakpoint (or size) a file shows: its name without the extension.
#[must_use]
pub fn breakpoint_of(name: &str) -> &str {
    name.rsplit_once('.').map_or(name, |(stem, _)| stem)
}

/// The folder of recording `id`; `None` when it is not there.
fn recording_dir(root: &Path, id: &str) -> Result<Option<PathBuf>, LibraryError> {
    if !is_recording_id(id) {
        return Err(LibraryError::Refused(id.to_owned()));
    }
    Ok(RunStore::open_in(root, &OUT_DIR, id, false)?.map(|store| store.dir().to_path_buf()))
}

fn io_error(path: &Path, error: &std::io::Error) -> LibraryError {
    LibraryError::Io(format!("{}: {error}", path.display()))
}

/// The video files of recording `id` in its folder `dir` that rBrowser wrote and nobody changed
/// since, by name: regular files only (a link is no file here).
fn videos_in(dir: &Path, id: &str, recorded: &Recorded) -> Result<Vec<VideoEntry>, LibraryError> {
    let entries = std::fs::read_dir(dir).map_err(|error| io_error(dir, &error))?;
    let mut files: Vec<VideoEntry> = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            if !is_video_name(&name) {
                return None;
            }
            let metadata = entry.metadata().ok()?;
            if !recorded.matches(id, &name, &metadata) {
                return None;
            }
            let poster = poster_name(&name)
                .filter(|poster| checked(&dir.join(poster), id, poster, recorded).is_ok());
            Some(VideoEntry {
                name,
                bytes: metadata.len(),
                modified: metadata.modified().ok(),
                poster,
            })
        })
        .collect();
    files.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(files)
}

/// The recordings of the project at `root` rBrowser wrote (`recorded`) that still hold one of
/// their files, newest first by when their folder was made (`sitemap_store::oldest_first`: a
/// name in local time does not sort by time across a daylight saving change, RB-090-69).
///
/// # Errors
///
/// A link on the way to the recordings' folder, or that folder cannot be read; a single
/// recording that cannot be read is logged and left out.
pub fn list(root: &Path, recorded: &Recorded) -> Result<Vec<RecordingEntry>, LibraryError> {
    let mut recordings = Vec::new();
    for id in run_ids_in(root, &OUT_DIR)?.into_iter().rev() {
        if !is_recording_id(&id) || !recorded.knows(&id) {
            continue;
        }
        // One recording that cannot be read is left out; the others are listed.
        let files = recording_dir(root, &id).and_then(|dir| match dir {
            Some(dir) => videos_in(&dir, &id, recorded).map(|files| Some((dir, files))),
            None => Ok(None),
        });
        match files {
            Ok(Some((dir, files))) if !files.is_empty() => {
                recordings.push(RecordingEntry { id, dir, files });
            }
            Ok(_) => {}
            Err(error) => {
                tracing::warn!(%error, recording = %id, "a recording cannot be read; it is left out");
            }
        }
    }
    Ok(recordings)
}

/// The folder of recording `id` (`file` `None`) or its video file `file`: a recording rBrowser
/// wrote, a real folder and a regular file of the recorder's stamp, reached without a link.
///
/// # Errors
///
/// A name that is no recording's or video's, a file changed since or a link or something else
/// on the way ([`LibraryError::Refused`]), or nothing there - a recording rBrowser did not write
/// included ([`LibraryError::NotFound`]).
pub fn locate(
    root: &Path,
    recorded: &Recorded,
    id: &str,
    file: Option<&str>,
) -> Result<PathBuf, LibraryError> {
    if let Some(name) = file
        && !is_video_name(name)
    {
        return Err(LibraryError::Refused(name.to_owned()));
    }
    locate_file(root, recorded, id, file)
}

/// [`locate`] for any file name the route serves, a poster included.
fn locate_file(
    root: &Path,
    recorded: &Recorded,
    id: &str,
    file: Option<&str>,
) -> Result<PathBuf, LibraryError> {
    if let Some(name) = file
        && !is_served_name(name)
    {
        return Err(LibraryError::Refused(name.to_owned()));
    }
    let dir = recording_dir(root, id)?.ok_or(LibraryError::NotFound)?;
    if !recorded.knows(id) {
        return Err(LibraryError::NotFound);
    }
    let Some(name) = file else {
        return Ok(dir);
    };
    let path = dir.join(name);
    checked(&path, id, name, recorded)?;
    Ok(path)
}

/// The metadata of `path` (no link followed) when it is the regular file rBrowser wrote.
fn checked(
    path: &Path,
    id: &str,
    name: &str,
    recorded: &Recorded,
) -> Result<Metadata, LibraryError> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_file() && recorded.matches(id, name, &metadata) => {
            Ok(metadata)
        }
        Ok(metadata) if metadata.file_type().is_file() && recorded.stamp(id, name).is_none() => {
            Err(LibraryError::NotFound)
        }
        Ok(_) => Err(LibraryError::Refused(name.to_owned())),
        Err(error) if error.kind() == ErrorKind::NotFound => Err(LibraryError::NotFound),
        Err(error) => Err(io_error(path, &error)),
    }
}

/// Opens the video file `name` of recording `id` - or a video's poster - for reading: located as
/// [`locate`] does, opened without following a link, and the open handle held to the same
/// checks - a regular file, the one the path showed, of the recorder's stamp.
///
/// # Errors
///
/// As [`locate`]; a handle that is not the file the path showed.
pub fn open_video(
    root: &Path,
    recorded: &Recorded,
    id: &str,
    name: &str,
) -> Result<(File, Metadata), LibraryError> {
    let path = locate_file(root, recorded, id, Some(name))?;
    let seen = std::fs::symlink_metadata(&path).map_err(|error| io_error(&path, &error))?;
    let file = no_follow::open(&path).map_err(|error| io_error(&path, &error))?;
    let opened = file.metadata().map_err(|error| io_error(&path, &error))?;
    if !opened.is_file()
        || !no_follow::same_file(&seen, &opened)
        || !recorded.matches(id, name, &opened)
    {
        return Err(LibraryError::Refused(name.to_owned()));
    }
    Ok((file, opened))
}

/// Deletes the video file `file` of recording `id` with its poster - unless another video of the
/// same breakpoint keeps it (a WebM and its MP4 export, RB-090-52) -, or every file of it
/// rBrowser wrote (`file` `None`); the folder goes when nothing else is left in it. Each file is checked
/// again right before it is removed; a poster that cannot go is logged and keeps the folder.
/// Answers the videos deleted and whether the folder went too. The recordings in `running`
/// (being written now) are refused.
///
/// # Errors
///
/// As [`locate`]; [`LibraryError::Running`]; a file that cannot be deleted (on Windows one a
/// player still holds open).
pub fn delete(
    root: &Path,
    recorded: &Recorded,
    id: &str,
    file: Option<&str>,
    running: &[&str],
) -> Result<Deleted, LibraryError> {
    if running.contains(&id) {
        return Err(LibraryError::Running);
    }
    let dir = locate(root, recorded, id, None)?;
    let names: Vec<String> = match file {
        Some(name) => {
            locate(root, recorded, id, Some(name))?;
            vec![name.to_owned()]
        }
        None => videos_in(&dir, id, recorded)?
            .into_iter()
            .map(|entry| entry.name)
            .collect(),
    };
    let mut deleted = Vec::new();
    for name in names {
        let path = dir.join(&name);
        checked(&path, id, &name, recorded)?;
        std::fs::remove_file(&path).map_err(|error| io_error(&path, &error))?;
        let keeps_poster = file.is_some() && recorded.poster_shared(id, &name);
        if !keeps_poster {
            remove_poster(&dir, id, poster_name(&name), recorded);
        }
        deleted.push(name);
    }
    if file.is_none() {
        // Posters whose video went before (one that could not be deleted with it).
        let posters = recorded.recordings.get(id).into_iter().flat_map(|files| {
            files
                .keys()
                .filter(|name| poster::is_poster_name(name))
                .cloned()
        });
        for poster in posters {
            remove_poster(&dir, id, Some(poster), recorded);
        }
    }
    // Fails while anything else is in it - which then stays, folder and all - and on Windows
    // while a player still holds a deleted file open.
    let folder_removed = std::fs::remove_dir(&dir).is_ok();
    Ok(Deleted {
        names: deleted,
        folder_removed,
    })
}

/// Removes the poster `poster` of recording `id` in its folder `dir` when it is the one rBrowser
/// wrote; nothing there is fine, anything else is logged.
fn remove_poster(dir: &Path, id: &str, poster: Option<String>, recorded: &Recorded) {
    let Some(poster) = poster.filter(|poster| recorded.stamp(id, poster).is_some()) else {
        return;
    };
    let path = dir.join(&poster);
    let removed = checked(&path, id, &poster, recorded)
        .and_then(|_| std::fs::remove_file(&path).map_err(|error| io_error(&path, &error)));
    match removed {
        Ok(()) | Err(LibraryError::NotFound) => {}
        Err(error) => tracing::warn!(%error, recording = %id, "a poster was not deleted"),
    }
}

/// Removes the folder of recording `id` when it is empty; `true` when it is gone (or never
/// was), `false` while anything is in it or it cannot be reached without a link.
#[must_use]
pub fn remove_empty_folder(root: &Path, id: &str) -> bool {
    match recording_dir(root, id) {
        Ok(None) => true,
        Ok(Some(dir)) => std::fs::remove_dir(dir).is_ok(),
        Err(_) => false,
    }
}

#[cfg(test)]
#[path = "library_tests.rs"]
mod tests;
