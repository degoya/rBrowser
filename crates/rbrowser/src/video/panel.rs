//! The Videos panel's core side (RB-110-12): a core panel of the panels page
//! (`crate::panel_dock::core_panels::VIDEOS`, `ui/src/videos/`) - the page's own rights, no
//! plugin frame - that lists the open project's recordings (`getVideos`), plays them from the
//! video route (`super::serve`, `rbrowser://video/...`), deletes what the person confirmed
//! (`deleteVideo`), shows a recording in the file manager (`revealVideo`) and exports a WebM as
//! MP4 or converts an old MP4 to WebM in the background (`convertVideo`, RB-090-52,
//! `super::convert_queue`). MCP lists them (`video_list`); it deletes nothing - a deletion is the
//! person's, after the panel's confirmation, and the retention settings are the agent's way to
//! keep the folder small. The export has no MCP tool: an agent works with the WebM the list
//! names; the MP4 is for a person to share.
//!
//! Only recordings rBrowser wrote count ([`super::registry`]): after every saved recording its
//! files are registered, the retention runs ([`on_saved`]) and the panel is told to ask again
//! (`videosChanged`). The registry folder is the user's local data folder; the `*_in` functions
//! take it as an argument, so tests never touch the real one.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

use rb_ipc::{
    ConvertVideoRequest, CoreToShell, IpcErrorCode, ShellToCore, VideoFile, VideoFileRef,
    VideoList, VideoRecording,
};

use super::convert::{ConvertError, mp4_encoder};
use serde_json::Value;

use super::library::{self, LibraryError, RecordingEntry};
use super::poster::poster_name;
use super::registry::{self, FileStamp, Recorded, Registry};
use super::serve::url_of;
use super::settings::OUT_DIR;
use crate::control::Core;
use crate::ipc::Failure;

/// The MCP tool that lists the recordings.
pub const LIST_TOOL: &str = "video_list";

/// The recordings rBrowser wrote (`recorded`) of the project at `root` (`None`: no project
/// open) as the panel shows them.
#[must_use]
pub fn video_list(root: Option<&Path>, recorded: &Recorded, autoplay: bool) -> VideoList {
    let Some(root) = root else {
        return VideoList {
            project: false,
            autoplay,
            ..VideoList::default()
        };
    };
    let mut folder = root.to_path_buf();
    folder.extend(OUT_DIR);
    let (recordings, error) = match library::list(root, recorded) {
        Ok(recordings) => (recordings, None),
        Err(error) => (Vec::new(), Some(error.to_string())),
    };
    VideoList {
        project: true,
        folder: Some(folder.display().to_string()),
        autoplay,
        error,
        recordings: recordings.iter().map(recording).collect(),
        mp4_export: mp4_encoder().is_some(),
        conversions: Vec::new(),
    }
}

fn recording(entry: &RecordingEntry) -> VideoRecording {
    VideoRecording {
        id: entry.id.clone(),
        path: entry.dir.display().to_string(),
        files: entry
            .files
            .iter()
            .map(|file| VideoFile {
                name: file.name.clone(),
                breakpoint: library::breakpoint_of(&file.name).to_owned(),
                bytes: file.bytes,
                path: entry.dir.join(&file.name).display().to_string(),
                url: url_of(&entry.id, &file.name),
                poster: file.poster.as_ref().map(|poster| url_of(&entry.id, poster)),
            })
            .collect(),
    }
}

/// The id of the recording being written now.
fn running_id() -> Option<String> {
    let folder = super::app::running_folder()?;
    folder.file_name()?.to_str().map(str::to_owned)
}

/// The recordings nothing may delete now: the one being written and those with an export or
/// conversion waiting or running (RB-090-52).
fn in_use() -> Vec<String> {
    let mut ids = super::convert_queue::busy_recordings();
    ids.extend(running_id());
    ids
}

/// The registry of the project at `root` in the folder `dir`; `None` without one.
fn registry_of(dir: Option<&Path>, root: &Path) -> Option<Registry> {
    dir.map(|dir| Registry::for_project(dir, root))
}

/// What `registry` holds; nothing without one.
fn recorded_of(registry: Option<&Registry>) -> Recorded {
    registry.map(Registry::load).unwrap_or_default()
}

/// The recordings of `core`'s project with the settings in effect, the registry in `dir`.
pub fn list_in(core: &Core, dir: Option<&Path>) -> VideoList {
    let autoplay = super::app::settings(core).autoplay;
    let root = core.host().project_root();
    let recorded = root
        .as_deref()
        .map(|root| {
            let registry = registry_of(dir, root);
            // Folders a deletion left behind go once they are empty.
            if let Some(registry) = &registry
                && !registry.load().pending.is_empty()
            {
                super::retention::settle_pending(root, registry);
            }
            recorded_of(registry.as_ref())
        })
        .unwrap_or_default();
    let mut list = video_list(root.as_deref(), &recorded, autoplay);
    if list.project {
        list.conversions = super::convert_queue::snapshot();
    }
    list
}

/// Answers the panel's `getVideos`, `deleteVideo`, `revealVideo` and `convertVideo`; `None` for
/// any other message.
pub fn ipc(core: &Core, message: &ShellToCore) -> Option<Result<CoreToShell, Failure>> {
    ipc_in(core, message, registry::default_dir().as_deref())
}

/// [`ipc`] with the registry in `dir`.
pub fn ipc_in(
    core: &Core,
    message: &ShellToCore,
    dir: Option<&Path>,
) -> Option<Result<CoreToShell, Failure>> {
    match message {
        ShellToCore::GetVideos => Some(Ok(CoreToShell::Videos(list_in(core, dir)))),
        ShellToCore::DeleteVideo(VideoFileRef { recording, file }) => {
            Some(delete(core, dir, recording, file.as_deref()))
        }
        ShellToCore::RevealVideo(VideoFileRef { recording, file }) => {
            Some(reveal(core, dir, recording, file.as_deref()))
        }
        ShellToCore::ConvertVideo(ConvertVideoRequest {
            recording,
            file,
            to,
        }) => Some(
            super::convert_queue::enqueue(core, dir, recording, file, *to)
                .map(|()| CoreToShell::Videos(list_in(core, dir)))
                .map_err(|error| {
                    tracing::warn!(%error, recording = %recording, "a video was not converted");
                    convert_failure(&error)
                }),
        ),
        _ => None,
    }
}

fn failure(error: &LibraryError) -> Failure {
    let code = match error {
        LibraryError::Refused(_) | LibraryError::Running => IpcErrorCode::Refused,
        LibraryError::NotFound => IpcErrorCode::NotFound,
        LibraryError::Io(_) => IpcErrorCode::Internal,
    };
    (code, error.to_string())
}

fn convert_failure(error: &ConvertError) -> Failure {
    match error {
        ConvertError::Library(error) => failure(error),
        ConvertError::Exists(_) | ConvertError::Queued | ConvertError::Unsupported => {
            (IpcErrorCode::Refused, error.to_string())
        }
        ConvertError::Ffmpeg(_) => (IpcErrorCode::Internal, error.to_string()),
    }
}

fn project_root(core: &Core) -> Result<PathBuf, Failure> {
    core.host().project_root().ok_or_else(|| {
        (
            IpcErrorCode::NotFound,
            super::VideoError::NoProject.to_string(),
        )
    })
}

/// Deletes what the person confirmed, forgets it in the registry and answers the list.
fn delete(
    core: &Core,
    dir: Option<&Path>,
    id: &str,
    file: Option<&str>,
) -> Result<CoreToShell, Failure> {
    let root = project_root(core)?;
    let registry = registry_of(dir, &root);
    let recorded = recorded_of(registry.as_ref());
    let in_use = in_use();
    let running: Vec<&str> = in_use.iter().map(String::as_str).collect();
    let deleted = library::delete(&root, &recorded, id, file, &running).map_err(|error| {
        tracing::warn!(%error, recording = %id, "a recording was not deleted");
        failure(&error)
    })?;
    if let Some(registry) = &registry {
        let forgotten = registry.forget_deleted(id, file, deleted.folder_removed);
        if let Err(error) = forgotten {
            tracing::warn!(%error, "the video registry was not updated");
        }
    }
    tracing::info!(count = deleted.names.len(), recording = %id, "recorded video deleted");
    Ok(CoreToShell::Videos(list_in(core, dir)))
}

/// Shows the recording or its file in the file manager, off the UI thread.
fn reveal(
    core: &Core,
    dir: Option<&Path>,
    id: &str,
    file: Option<&str>,
) -> Result<CoreToShell, Failure> {
    let root = project_root(core)?;
    let recorded = recorded_of(registry_of(dir, &root).as_ref());
    let path = library::locate(&root, &recorded, id, file).map_err(|error| failure(&error))?;
    core.host().runtime().spawn_blocking(move || {
        if let Err(error) = rb_diag::reveal(&path) {
            tracing::warn!(%error, "a recording cannot be shown in the file manager");
        }
    });
    Ok(CoreToShell::Ack)
}

/// The project root and the recording id of a recording's folder
/// (`<root>/.rbrowser/out/video/<id>`); `None` for another path.
#[must_use]
pub fn project_of(folder: &Path) -> Option<(PathBuf, String)> {
    let id = folder.file_name()?.to_str()?.to_owned();
    let mut root = folder.parent()?;
    for part in OUT_DIR.iter().rev() {
        if root.file_name()?.to_str()? != *part {
            return None;
        }
        root = root.parent()?;
    }
    Some((root.to_path_buf(), id))
}

/// Registers the files `names` the recorder just wrote into `folder` - each a regular file,
/// stamped as it is now - in the registry of its project in `dir`: its videos, and a poster
/// only beside its video ([`super::poster`]).
///
/// # Errors
///
/// The folder is no recording's, or the registry cannot be written.
pub fn register_saved(dir: &Path, folder: &Path, names: &[String]) -> std::io::Result<()> {
    let (root, id) =
        project_of(folder).ok_or_else(|| std::io::Error::other("the folder is no recording's"))?;
    let poster_of_one = |name: &str| {
        names
            .iter()
            .any(|video| poster_name(video).as_deref() == Some(name))
    };
    let files: Vec<(String, FileStamp)> = names
        .iter()
        .filter(|name| library::is_video_name(name) || poster_of_one(name.as_str()))
        .filter_map(|name| {
            let metadata = std::fs::symlink_metadata(folder.join(name)).ok()?;
            if !metadata.file_type().is_file() {
                return None;
            }
            Some((name.clone(), FileStamp::of(&metadata)?))
        })
        .collect();
    if files.is_empty() {
        return Ok(());
    }
    Registry::for_project(dir, &root).record(&id, &files)
}

/// A recording was saved to `folder` with the files `names`: their posters are made
/// ([`super::poster`], with the ffmpeg that recorded them), all is registered, the retention
/// runs (never on that recording nor on one being written) and the panel is told to ask
/// again - off the caller's thread.
pub fn on_saved(core: &Arc<Core>, folder: &Path, names: Vec<String>) {
    let settings = super::app::settings(core);
    let ffmpeg = super::ffmpeg::locate(&settings, super::ffmpeg::default_store().as_ref());
    let (keep, keep_days) = (settings.keep_recordings, settings.keep_days);
    let folder = folder.to_path_buf();
    let notifier = Arc::clone(core);
    let runtime = core.host().runtime().clone();
    core.host().runtime().spawn(async move {
        let mut names = names;
        if let Some(ffmpeg) = ffmpeg {
            let posters = super::poster::write_all(&ffmpeg.path, &folder, &names).await;
            names.extend(posters);
        }
        let registered =
            runtime.spawn_blocking(move || register_and_keep(&folder, &names, keep, keep_days));
        if let Err(error) = registered.await {
            tracing::warn!(%error, "the recording was not registered for the Videos panel");
        }
        notifier.send(&CoreToShell::VideosChanged);
    });
}

/// Registers the files `names` of the recording in `folder` and applies the retention.
fn register_and_keep(folder: &Path, names: &[String], keep: u32, keep_days: u32) {
    let dir = registry::default_dir();
    let (Some(dir), Some((root, id))) = (dir.as_deref(), project_of(folder)) else {
        return;
    };
    if let Err(error) = register_saved(dir, folder, names) {
        tracing::warn!(%error, "the recording was not registered for the Videos panel");
    }
    let in_use = in_use();
    let mut protect = vec![id.as_str()];
    protect.extend(in_use.iter().map(String::as_str));
    let registry = Registry::for_project(dir, &root);
    match super::retention::apply(
        &root,
        &registry,
        keep,
        keep_days,
        SystemTime::now(),
        &protect,
    ) {
        Ok(deleted) if !deleted.is_empty() => {
            tracing::info!(count = deleted.len(), "old recordings deleted (retention)");
        }
        Ok(_) => {}
        Err(error) => tracing::warn!(%error, "the recordings' retention did not run"),
    }
}

/// The arguments of `video_list`: none.
///
/// # Errors
///
/// Anything but `null` or an empty object.
pub fn tool_args(args: &Value) -> Result<(), rb_mcp::ToolFailure> {
    match args {
        Value::Null => Ok(()),
        Value::Object(fields) if fields.is_empty() => Ok(()),
        _ => Err(rb_mcp::ToolFailure::new(
            "invalid_arguments",
            "video_list takes no arguments",
        )),
    }
}

/// The MCP tool `video_list`: the recordings rBrowser wrote, newest first, each file with its
/// breakpoint, size and path. A read; it deletes nothing.
///
/// # Errors
///
/// Arguments where there are none.
pub fn host_tool(core: &Core, args: &Value) -> Result<Value, rb_mcp::ToolFailure> {
    tool_args(args)?;
    serde_json::to_value(list_in(core, registry::default_dir().as_deref()))
        .map_err(|error| rb_mcp::ToolFailure::new("internal", error.to_string()))
}

#[cfg(test)]
#[path = "panel_tests.rs"]
mod tests;
