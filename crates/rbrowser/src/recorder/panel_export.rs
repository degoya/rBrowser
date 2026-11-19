//! The Recorder panel's export (`exportRecording`, RB-090-60) and where "Save" puts it
//! (RB-090-66): the flow into the flows folder (`<automation.flows.dir>/<name>.ts`, [`flow_file`],
//! so the Flows panel lists it and the project's trust holds for it), the Playwright test and
//! its config into a new folder of `automation.recorderExport.dir`. The preview names the flow's
//! file and whether one of that name exists; the dialog asks before it is replaced. A save ends
//! with a notice that names the files; the dialog closes.

use std::path::Path;
use std::time::SystemTime;

use rb_ipc::{
    CoreToShell, IpcErrorCode, Notification, NotificationLevel, RecordingExport,
    RecordingExportFile, RecordingExportFormat,
};
use rb_sync::recorder::check_name;

use super::export::{self, ExportFormat};
use super::flow_file::{self, FlowFileError, SavedFlow, Target};
use super::glue;
use super::panel::{command_failure, format_of, internal, store_failure};
use crate::control::Core;
use crate::ipc::Failure;

/// The notice after a save; `{files}` the files written, relative to the project root.
pub const SAVED_NOTICE: &str = "recorder.export.notice.saved";

/// What the panel asks for.
#[derive(Clone, Copy, Debug)]
pub struct Request<'a> {
    /// The recording.
    pub name: &'a str,
    /// What to make.
    pub format: RecordingExportFormat,
    /// The export's name; the recording's when absent or empty.
    pub export_name: Option<&'a str>,
    /// Write the files; a preview without.
    pub write: bool,
    /// Replace a flow of the same name (the person said so in the dialog).
    pub overwrite: bool,
}

/// Answers `exportRecording`.
///
/// # Errors
///
/// A bad name, no such recording, no project, the plugin failed, an existing flow without
/// `overwrite`, or a file that cannot be written.
pub fn export_recording(core: &Core, request: &Request<'_>) -> Result<CoreToShell, Failure> {
    let store = glue::store_of(core.host()).map_err(|error| internal(&error))?;
    let recording = store.load(request.name).map_err(store_failure)?;
    // The export's name names the files; the text still names the recording (RB-090-69).
    let export_name = request.export_name.filter(|text| !text.is_empty());
    if let Some(export_name) = export_name {
        check_name(export_name).map_err(store_failure)?;
    }
    let name = export_name.unwrap_or(&recording.name);
    let root = core
        .host()
        .project_root()
        .ok_or_else(|| (IpcErrorCode::BadRequest, "no project is open".to_owned()))?;
    let format = format_of(request.format);
    let flows_dir = core.user_settings().automation.flows.dir;
    let store_path = crate::trust::store_path(core);
    let now = crate::trust::rfc3339(SystemTime::now());
    let target = Target {
        root: &root,
        dir: &flows_dir,
        name,
        store: store_path.as_deref(),
        now: &now,
    };
    if !request.write {
        let files = export::render_recording(core, &recording, export_name, format)
            .map_err(command_failure)?;
        let flow = (format != ExportFormat::Spec)
            .then(|| flow_file::path_of(&target).ok())
            .flatten();
        return Ok(CoreToShell::RecordingExport(RecordingExport {
            written: false,
            folder: None,
            files: files_of(files),
            flow_file: flow
                .as_ref()
                .map(|path| crate::flows::catalog::relative(&root, path)),
            flow_exists: flow.as_deref().is_some_and(Path::exists),
        }));
    }
    let mut written = Vec::new();
    let mut folder = None;
    if format != ExportFormat::Spec {
        let source = export::render_recording(core, &recording, export_name, ExportFormat::Flow)
            .map_err(command_failure)?
            .into_iter()
            .next()
            .map(|(_, text)| text)
            .ok_or_else(|| internal(&"the recorder-export plugin made no flow"))?;
        let saved = flow_file::write_flow(&target, &source, request.overwrite)
            .map_err(|error| flow_failure(&error))?;
        after_flow_saved(core, &saved);
        folder = saved.path.rsplit_once('/').map(|(dir, _)| dir.to_owned());
        written.push(RecordingExportFile {
            path: saved.path,
            content: source,
        });
    }
    if format != ExportFormat::Flow {
        let files = export::render_recording(core, &recording, export_name, ExportFormat::Spec)
            .map_err(command_failure)?;
        let answer = export::write_export(core, &root, request.name.to_owned(), name, files)
            .map_err(command_failure)?;
        folder = Some(answer.folder);
        written.extend(answer.files.into_iter().map(|file| RecordingExportFile {
            path: file.path,
            content: file.content,
        }));
    }
    core.notify(saved_notice(&written));
    Ok(CoreToShell::RecordingExport(RecordingExport {
        written: true,
        folder,
        files: written,
        flow_file: None,
        flow_exists: false,
    }))
}

/// A saved flow: the trust check runs again (it asks when the decision did not move with the
/// file), and the Flows panel lists again.
fn after_flow_saved(core: &Core, saved: &SavedFlow) {
    if !saved.trust_kept {
        tracing::info!("the saved flow changed the protected parts; workspace trust asks again");
    }
    crate::trust::on_config_changed(core);
    core.send(&CoreToShell::FlowRunChanged);
}

fn files_of(files: Vec<(String, String)>) -> Vec<RecordingExportFile> {
    files
        .into_iter()
        .map(|(path, content)| RecordingExportFile { path, content })
        .collect()
}

/// A flow that was not written, as the panel shows it: an existing flow is refused (the dialog
/// asks the person), a bad name or folder is a bad request.
#[must_use]
pub fn flow_failure(error: &FlowFileError) -> Failure {
    let code = match error {
        FlowFileError::BadName(_) | FlowFileError::BadDir(_) => IpcErrorCode::BadRequest,
        FlowFileError::Exists(_) | FlowFileError::Linked(_) => IpcErrorCode::Refused,
        FlowFileError::Io(_) => IpcErrorCode::Internal,
    };
    (code, error.to_string())
}

/// The notice that names where the files went.
#[must_use]
pub fn saved_notice(files: &[RecordingExportFile]) -> CoreToShell {
    let paths: Vec<&str> = files.iter().map(|file| file.path.as_str()).collect();
    saved_paths_notice(&paths)
}

/// [`saved_notice`] of the files at `paths` - also `recorder.export_flow` and
/// `recorder.export_spec`'s (RB-090-84).
#[must_use]
pub fn saved_paths_notice(paths: &[&str]) -> CoreToShell {
    CoreToShell::Notification(Notification {
        level: NotificationLevel::Info,
        message_key: SAVED_NOTICE.to_owned(),
        args: [("files".to_owned(), paths.join(", "))]
            .into_iter()
            .collect(),
        settings_key: None,
    })
}

#[cfg(test)]
#[path = "panel_export_tests.rs"]
mod tests;
