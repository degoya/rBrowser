//! The Recorder panel's requests (RB-090-60): the stored recordings (`getRecordings`), one as
//! its file holds it (`getRecording`), the edited one written back (`saveRecording`) and its
//! export (`exportRecording`: a preview, or the files written - the flow into the flows folder,
//! the test into a new export folder, [`super::panel_export`]). All
//! four run off the UI thread ([`dispatch_off_thread`]): they read the disk - a WSL project's is
//! a network share - and the export waits for the `recorder-export` plugin. `recordingsChanged`
//! follows a save and a stop, so an open panel asks again. `resendRecordingSteps` (RB-090-89) runs
//! off it too: it waits for the recorder, which a stop holds while it writes the file.

use std::path::Path;
use std::time::UNIX_EPOCH;

use rb_ipc::{
    CoreToShell, IpcErrorCode, RecordingEntry, RecordingExportFormat, RecordingList, ShellToCore,
};
use rb_sync::recorder::{Recording, RecordingStore, StoreError, check_name, from_json};
use serde_json::Value;

use super::export::ExportFormat;
use super::glue;
use crate::control::Core;
use crate::ipc::Failure;

/// Answers the panel's requests; `None` for any other message. The shell's queries come
/// through [`dispatch_off_thread`]: every one reads the disk, the export waits for the plugin.
pub fn ipc(core: &Core, message: &ShellToCore) -> Option<Result<CoreToShell, Failure>> {
    Some(match message {
        ShellToCore::GetRecordings => Ok(CoreToShell::Recordings(list(core))),
        // A panel opened while a recording runs catches up (RB-090-89).
        ShellToCore::ResendRecordingSteps => {
            super::live::resend(core);
            Ok(CoreToShell::Ack)
        }
        ShellToCore::GetRecording { name } => get(core, name),
        ShellToCore::SaveRecording { name, recording } => save(core, name, recording),
        ShellToCore::ExportRecording {
            name,
            format,
            export_name,
            write,
            overwrite,
        } => super::panel_export::export_recording(
            core,
            &super::panel_export::Request {
                name,
                format: *format,
                export_name: export_name.as_deref(),
                write: *write,
                overwrite: *overwrite,
            },
        ),
        _ => return None,
    })
}

/// Whether `message` is one of the panel's, or the Flows panel's `getFlowRun` (which reads the
/// last `result.json`): [`dispatch_off_thread`] answers it.
#[must_use]
pub fn is_off_thread(message: &ShellToCore) -> bool {
    matches!(
        message,
        ShellToCore::GetRecordings
            | ShellToCore::GetRecording { .. }
            | ShellToCore::SaveRecording { .. }
            | ShellToCore::ExportRecording { .. }
            | ShellToCore::GetFlowRun
            | ShellToCore::ResendRecordingSteps
    )
}

/// Answers the panel's `message` on a blocking thread of the core's runtime.
pub fn dispatch_off_thread(
    core: &Core,
    message: ShellToCore,
    reply: impl FnOnce(Result<CoreToShell, Failure>) + Send + 'static,
) {
    let Some(core) = core.arc() else {
        reply(Err((
            IpcErrorCode::Internal,
            "the window is closed".to_owned(),
        )));
        return;
    };
    let runtime = core.runtime().clone();
    runtime.spawn_blocking(move || {
        let answer = ipc(&core, &message).or_else(|| crate::flows::panel::ipc(&core, &message));
        reply(answer.unwrap_or_else(|| {
            Err((
                IpcErrorCode::BadRequest,
                format!("{message:?} is no Recorder panel request"),
            ))
        }));
    });
}

/// The stored recordings, newest first, with the one running now.
pub fn list(core: &Core) -> RecordingList {
    let host = core.host();
    let Some(root) = host.project_root() else {
        return RecordingList::default();
    };
    let recording_now = super::for_host(host).and_then(|recorder| recorder.outcome().name);
    let store = match glue::store_of(host) {
        Ok(store) => store,
        Err(error) => {
            return RecordingList {
                project: true,
                error: Some(error.to_string()),
                recording_now,
                ..RecordingList::default()
            };
        }
    };
    let folder = Some(relative(&root, store.dir()));
    match entries(&store) {
        Ok(recordings) => RecordingList {
            project: true,
            folder,
            error: None,
            recordings,
            recording_now,
        },
        Err(error) => RecordingList {
            project: true,
            folder,
            error: Some(error.to_string()),
            recordings: Vec::new(),
            recording_now,
        },
    }
}

/// The recordings of `store`, newest first.
///
/// # Errors
///
/// The folder cannot be read.
pub fn entries(store: &RecordingStore) -> Result<Vec<RecordingEntry>, StoreError> {
    let mut entries: Vec<RecordingEntry> = store
        .list()?
        .into_iter()
        .map(|name| {
            let modified_ms = store
                .path_of(&name)
                .ok()
                .and_then(|path| path.metadata().ok())
                .and_then(|meta| meta.modified().ok())
                .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                .map_or(0, |since| {
                    u64::try_from(since.as_millis()).unwrap_or(u64::MAX)
                });
            let steps = store.load(&name).map_or(0, |recording| {
                u32::try_from(recording.steps.len()).unwrap_or(u32::MAX)
            });
            RecordingEntry {
                name,
                steps,
                modified_ms,
            }
        })
        .collect();
    newest_first(&mut entries);
    Ok(entries)
}

/// Newest first; the same time by name.
pub fn newest_first(entries: &mut [RecordingEntry]) {
    entries.sort_by(|a, b| {
        b.modified_ms
            .cmp(&a.modified_ms)
            .then_with(|| a.name.cmp(&b.name))
    });
}

fn get(core: &Core, name: &str) -> Result<CoreToShell, Failure> {
    let store = glue::store_of(core.host()).map_err(|error| internal(&error))?;
    let recording = store.load(name).map_err(store_failure)?;
    // The pauses Play waits, from the exporter (RB-090-84); the recording shows without them.
    let pauses = super::export::recorded_pauses(core, &recording).unwrap_or_else(|error| {
        tracing::debug!(%error, "the recording's pauses are not known");
        Vec::new()
    });
    let recording = serde_json::to_value(&recording).map_err(|error| internal(&error))?;
    Ok(CoreToShell::Recording {
        name: name.to_owned(),
        recording,
        pauses,
    })
}

/// The panel's edited recording, checked: the model's schema (`recording.v1`), its name the
/// file's, its step ids unique, its pauses and waits within their cap.
///
/// # Errors
///
/// What is wrong, safe to show.
pub fn edited(name: &str, value: &Value) -> Result<Recording, String> {
    check_name(name).map_err(|error| error.to_string())?;
    let recording = from_json(&value.to_string()).map_err(|error| error.to_string())?;
    if recording.name != name {
        return Err(format!(
            "the recording is named `{}`, its file `{name}`",
            recording.name
        ));
    }
    // Unique ids, the editor's pauses and waits at most 10 s (RB-092-01).
    rb_sync::recorder::edit::check(&recording).map_err(|error| error.to_string())?;
    Ok(recording)
}

fn save(core: &Core, name: &str, value: &Value) -> Result<CoreToShell, Failure> {
    let recording = edited(name, value).map_err(|error| (IpcErrorCode::BadRequest, error))?;
    let store = glue::store_of(core.host()).map_err(|error| internal(&error))?;
    // Never between another writer's read and write (an agent's step tools, RB-092-01).
    let written = rb_sync::recorder::write_lock();
    store.replace(&recording).map_err(store_failure)?;
    drop(written);
    tracing::info!(steps = recording.steps.len(), "recording edited");
    core.send(&CoreToShell::RecordingsChanged);
    Ok(CoreToShell::Recordings(list(core)))
}

/// The panel's format as the export's.
#[must_use]
pub fn format_of(format: RecordingExportFormat) -> ExportFormat {
    match format {
        RecordingExportFormat::Flow => ExportFormat::Flow,
        RecordingExportFormat::Spec => ExportFormat::Spec,
        RecordingExportFormat::Both => ExportFormat::Both,
    }
}

pub(super) fn internal(error: &impl std::fmt::Display) -> Failure {
    (IpcErrorCode::Internal, error.to_string())
}

pub(super) fn store_failure(error: StoreError) -> Failure {
    let code = match error {
        StoreError::BadName(_) => IpcErrorCode::BadRequest,
        StoreError::NotFound(_) => IpcErrorCode::NotFound,
        _ => IpcErrorCode::Internal,
    };
    (code, error.to_string())
}

pub(super) fn command_failure(error: rb_commands::CommandError) -> Failure {
    let code = match error {
        rb_commands::CommandError::InvalidArgs(_) => IpcErrorCode::BadRequest,
        _ => IpcErrorCode::Internal,
    };
    (code, error.to_string())
}

/// `path` relative to `root`, with `/`.
fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

#[cfg(test)]
#[path = "panel_tests.rs"]
mod tests;
