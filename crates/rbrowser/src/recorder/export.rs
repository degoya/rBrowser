//! The recorder's export (RB-050-02): a stored recording as an rBrowser flow and a Playwright
//! test through the first-party plugin `recorder-export`. The host reads the recording, adds the
//! locator preference, the environment's origin and one Playwright project per breakpoint with
//! its resolved emulation ([`ExportRequest`]), lets the plugin render each file and writes them
//! into a new folder of `automation.recorderExport.dir` (default `.rbrowser/out/exports/<name>/`)
//! - never over an existing export, never into a folder workspace trust protects.
//!
//! - `recorder.export_flow` (`Leader F W`) and `recorder.export_spec` (`Leader F E`):
//!   `{ recordingId?, name? }`, the last saved recording when `recordingId` is absent; both
//!   `covered(recorder_export)`.
//! - MCP `recorder_export` (RB-030-01's server registers it): [`RecorderExportArgs`] ->
//!   [`RecorderExportAnswer`], the files' paths and content. A recording holds no secret in
//!   clear (`{{secret:<name>}}`), and the code reads `RB_SECRET_<NAME>` from the environment,
//!   stopping with the variable's name when it is not set.
//!
//! The plugin runs on a thread of its own: a command may come from the CEF UI thread (outside
//! the runtime) or from inside the runtime, and the plugin call blocks on a runtime of its own.

use std::path::Path;
use std::time::SystemTime;

use rb_commands::CommandError;
use rb_config::{Breakpoint, PlaybackPace, ProjectConfig};
use rb_engine::{Emulation, Target};
use rb_ipc::StepPause;
use rb_sync::recorder::{
    ExportOptions, ExportProject, ExportRequest, ExportsDir, ExportsError, Recording,
    RecordingStore, StoreError, check_name,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::runtime::Handle;

use super::glue;
use crate::control::Core;

/// `Leader F W`: export the last (or the named) recording as a flow.
pub const EXPORT_FLOW: &str = "recorder.export_flow";
/// `Leader F E`: export the last (or the named) recording as a Playwright test.
pub const EXPORT_SPEC: &str = "recorder.export_spec";
/// The exporter plugin's id.
pub const PLUGIN_ID: &str = "recorder-export";
/// `recorder_export`'s name.
pub const RECORDER_EXPORT_TOOL: &str = "recorder_export";
/// `recorder_export`'s description for the MCP tool list.
pub const RECORDER_EXPORT_DESCRIPTION: &str = "Export a stored recording as an rBrowser flow \
     (`format: \"flow\"`, `<name>.ts` with `page`, `view` and `expect` injected), as a \
     Playwright test (`\"spec\"`, `<name>.spec.ts` plus `playwright.config.ts` with one project \
     per breakpoint: viewport, device scale factor, mobile, touch, user agent, locale, colour \
     scheme) or both (`\"both\"`, the default). `recordingId` names the recording \
     (`recorder_stop` answers it); the last saved one when absent. `name` renames the export. \
     The files go into a new folder of `automation.recorderExport.dir` (default \
     `.rbrowser/out/exports/<name>/`); nothing is overwritten. Secrets stay placeholders: the \
     code reads `RB_SECRET_<NAME>` from the environment and stops when it is not set. Answers the folder and each file's path and \
     content.";

/// What `recorder_export` writes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    /// The rBrowser flow.
    Flow,
    /// The Playwright test and, with `automation.recorderExport.includeConfig`, its config.
    Spec,
    /// Both.
    #[default]
    Both,
}

/// The arguments of `recorder_export`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RecorderExportArgs {
    /// `"all"` (absent); the recorder belongs to the window, not a view.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<Target>,
    /// The recording's name; the last saved one when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recording_id: Option<String>,
    /// What to write.
    #[serde(default)]
    pub format: ExportFormat,
    /// The export's name (`[A-Za-z0-9_-]`, at most 64 characters); the recording's when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// One written file.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportedFile {
    /// Relative to the project root, with `/`.
    pub path: String,
    /// The file's text.
    pub content: String,
}

/// The answer of `recorder_export` and both commands.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecorderExportAnswer {
    /// The recording exported.
    pub recording_id: String,
    /// The export's folder, relative to the project root.
    pub folder: String,
    /// The files written.
    pub files: Vec<ExportedFile>,
}

/// Runs `recorder.export_flow` or `recorder.export_spec`: `args` with the command's format. The
/// notice names the files written, as the panel's export does (RB-090-84).
pub fn run_export_command(
    core: &Core,
    format: ExportFormat,
    args: &Value,
) -> Result<RecorderExportAnswer, CommandError> {
    let answer = recorder_export(core, &with_format(args, format))?;
    let paths: Vec<&str> = answer.files.iter().map(|file| file.path.as_str()).collect();
    core.notify(super::panel_export::saved_paths_notice(&paths));
    Ok(answer)
}

/// `args` (an object or null) with `format` set.
pub fn with_format(args: &Value, format: ExportFormat) -> Value {
    let mut object = args.as_object().cloned().unwrap_or_default();
    object.insert(
        "format".to_owned(),
        serde_json::to_value(format).unwrap_or(Value::Null),
    );
    Value::Object(object)
}

/// Runs `recorder_export` with `args` (its JSON).
pub fn recorder_export(core: &Core, args: &Value) -> Result<RecorderExportAnswer, CommandError> {
    let args: RecorderExportArgs = if args.is_null() {
        RecorderExportArgs::default()
    } else {
        serde_json::from_value(args.clone())
            .map_err(|error| CommandError::InvalidArgs(error.to_string()))?
    };
    if let Some(Target::View(_)) = args.target {
        return Err(CommandError::InvalidArgs(
            "the recorder belongs to the window: use `target: \"all\"`".to_owned(),
        ));
    }
    let root = core
        .host()
        .project_root()
        .ok_or_else(|| CommandError::Failed("no project is open".to_owned()))?;
    let store =
        glue::store_of(core.host()).map_err(|error| CommandError::Failed(error.to_string()))?;
    let id = match args.recording_id {
        Some(id) => id,
        None => latest(&store)?.ok_or_else(|| {
            CommandError::InvalidArgs(
                "no recording to export yet: record one, or pass `recordingId`".to_owned(),
            )
        })?,
    };
    let recording = store.load(&id).map_err(store_failed)?;
    if let Some(name) = &args.name {
        check_name(name).map_err(store_failed)?;
    }
    let files = render_recording(core, &recording, args.name.as_deref(), args.format)?;
    let name = args.name.as_deref().unwrap_or(&recording.name);
    write_export(core, &root, id, name, files)
}

/// `recording` rendered as `format` by the plugin: each file's name and text, nothing written
/// (also the Recorder panel's preview, RB-090-60), at the pace of
/// `automation.recorder.playbackPace`. The files are named `export_name` (the recording's name
/// when `None`); the text names the recording (RB-090-69).
pub fn render_recording(
    core: &Core,
    recording: &Recording,
    export_name: Option<&str>,
    format: ExportFormat,
) -> Result<Vec<(String, String)>, CommandError> {
    render_recording_at(core, recording, export_name, format, None)
}

/// [`render_recording`] at `pace`; the setting's when `None` (Play's choice, RB-090-66).
pub fn render_recording_at(
    core: &Core,
    recording: &Recording,
    export_name: Option<&str>,
    format: ExportFormat,
    pace: Option<PlaybackPace>,
) -> Result<Vec<(String, String)>, CommandError> {
    let config = core.host().config();
    let request = request_json(recording, export_name, &config, pace_of(pace, &config))?;
    let formats = plugin_formats(format, config.automation.recorder_export.include_config);
    render_all(core.runtime().clone(), &formats, &request)
}

/// The plugin's internal format with the pause before each step (`recorder-export`'s `pauses`).
pub const PAUSES_FORMAT: &str = "pauses";

/// The pause before each written step of `recording` at the `recorded` pace with
/// `automation.recorder.minPauseMs`, as the exporter writes them into the flow (RB-090-84): the
/// one rule, for a played flow's timeout and the Recorder panel.
///
/// # Errors
///
/// The plugin failed or answered no pauses.
pub fn recorded_pauses(core: &Core, recording: &Recording) -> Result<Vec<StepPause>, CommandError> {
    let config = core.host().config();
    let request = request_json(recording, None, &config, PlaybackPace::Recorded)?;
    let files = render_all(core.runtime().clone(), &[PAUSES_FORMAT], &request)?;
    let text = files.first().map_or("", |(_, text)| text.as_str());
    parse_pauses(text)
}

/// The plugin's `{"pauses":[{"step":1,"ms":0},...]}`.
///
/// # Errors
///
/// The text is no such list.
pub fn parse_pauses(text: &str) -> Result<Vec<StepPause>, CommandError> {
    #[derive(Deserialize)]
    struct Answer {
        pauses: Vec<StepPause>,
    }
    serde_json::from_str::<Answer>(text)
        .map(|answer| answer.pauses)
        .map_err(|error| CommandError::Failed(format!("the plugin's pauses: {error}")))
}

/// Writes `files` into a new folder `name` of `automation.recorderExport.dir` below `root`.
pub fn write_export(
    core: &Core,
    root: &Path,
    recording_id: String,
    name: &str,
    files: Vec<(String, String)>,
) -> Result<RecorderExportAnswer, CommandError> {
    let dir = core.host().config().automation.recorder_export.dir;
    let protected: Vec<&str> = rb_config::trust::PROTECTED_DIRS
        .iter()
        .map(|(_, name)| *name)
        .collect();
    let exported = ExportsDir::open(root, &dir, &protected)
        .and_then(|dir| dir.write_new(name, &files))
        .map_err(exports_failed)?;
    tracing::info!(files = exported.files.len(), "recording exported");
    Ok(RecorderExportAnswer {
        recording_id,
        folder: relative(root, &exported.folder),
        files: exported
            .files
            .iter()
            .zip(files)
            .map(|(path, (_, content))| ExportedFile {
                path: relative(root, path),
                content,
            })
            .collect(),
    })
}

/// The plugin's formats for `format`: `flow`, `spec` and - with `include_config` - the config.
pub fn plugin_formats(format: ExportFormat, include_config: bool) -> Vec<&'static str> {
    let mut formats = Vec::new();
    if format != ExportFormat::Spec {
        formats.push("flow");
    }
    if format != ExportFormat::Flow {
        formats.push("spec");
        if include_config {
            formats.push("playwright-config");
        }
    }
    formats
}

/// `asked`, else `automation.recorder.playbackPace`.
#[must_use]
pub fn pace_of(asked: Option<PlaybackPace>, config: &ProjectConfig) -> PlaybackPace {
    asked.unwrap_or(config.automation.recorder.playback_pace)
}

/// The request the plugin gets, as JSON; `export_name` names the files, never the recording.
pub fn request_json(
    recording: &Recording,
    export_name: Option<&str>,
    config: &ProjectConfig,
    pace: PlaybackPace,
) -> Result<String, CommandError> {
    let request = ExportRequest {
        recording,
        options: ExportOptions {
            locator_preference: config
                .automation
                .recorder_export
                .locator_preference
                .as_str()
                .to_owned(),
            base_url: base_url_of(recording, config),
            projects: projects_of(recording, config),
            pace: pace.as_str().to_owned(),
            min_pause_ms: config.automation.recorder.min_pause_ms,
            name: export_name.map(str::to_owned),
        },
    };
    serde_json::to_string(&request).map_err(|error| CommandError::Failed(error.to_string()))
}

/// The origin of the recording's environment, when the config still has it.
pub fn base_url_of(recording: &Recording, config: &ProjectConfig) -> Option<String> {
    let id = recording.environment.as_deref()?;
    config
        .environments
        .iter()
        .find(|environment| environment.id == id)
        .map(|environment| environment.url.clone())
}

/// One project per breakpoint of the recording (all of the config's when it names none): the
/// config's breakpoint with its resolved emulation, else the recorded size and pixel ratio.
pub fn projects_of(recording: &Recording, config: &ProjectConfig) -> Vec<ExportProject> {
    let configured = |id: &str| config.breakpoints.iter().find(|bp| bp.id == id);
    if recording.breakpoints.is_empty() {
        return config
            .breakpoints
            .iter()
            .map(|breakpoint| project_of(breakpoint, config))
            .collect();
    }
    recording
        .breakpoints
        .iter()
        .map(|recorded| match configured(&recorded.id) {
            Some(breakpoint) => project_of(breakpoint, config),
            None => ExportProject {
                name: recorded.id.clone(),
                engine: "chromium".to_owned(),
                width: recorded.width,
                height: recorded.height,
                device_scale_factor: (recorded.device_scale_factor > 0.0)
                    .then_some(recorded.device_scale_factor),
                ..ExportProject::default()
            },
        })
        .collect()
}

fn project_of(breakpoint: &Breakpoint, config: &ProjectConfig) -> ExportProject {
    let emulation: Emulation = crate::host::state::emulation_for(breakpoint, &config.emulation);
    ExportProject {
        name: breakpoint.id.clone(),
        engine: word(&breakpoint.engine).unwrap_or_else(|| "chromium".to_owned()),
        width: emulation.width,
        height: emulation.height,
        device_scale_factor: (emulation.dpr > 0.0).then_some(emulation.dpr),
        is_mobile: Some(emulation.mobile),
        has_touch: Some(emulation.touch),
        user_agent: emulation.user_agent.clone(),
        locale: emulation.locale.clone(),
        color_scheme: emulation.color_scheme.as_ref().and_then(word),
        reduced_motion: emulation.reduced_motion,
    }
}

/// A unit enum as the config writes it (`chromium`, `dark`).
fn word<T: Serialize>(value: &T) -> Option<String> {
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
}

/// The most recently written recording.
pub(super) fn latest(store: &RecordingStore) -> Result<Option<String>, CommandError> {
    let names = store.list().map_err(store_failed)?;
    Ok(names
        .into_iter()
        .map(|name| {
            let modified = store
                .path_of(&name)
                .ok()
                .and_then(|path| path.metadata().ok())
                .and_then(|meta| meta.modified().ok())
                .unwrap_or(SystemTime::UNIX_EPOCH);
            (modified, name)
        })
        .max()
        .map(|(_, name)| name))
}

/// Renders each format with the plugin, on a thread of its own that borrows `runtime`, the
/// core's, for the plugin calls.
fn render_all(
    runtime: Handle,
    formats: &[&str],
    request: &str,
) -> Result<Vec<(String, String)>, CommandError> {
    std::thread::scope(|scope| {
        scope
            .spawn(|| {
                let _runtime = runtime.enter();
                formats
                    .iter()
                    .map(|format| render(format, request))
                    .collect::<Result<Vec<_>, _>>()
            })
            .join()
            .unwrap_or_else(|_| Err(CommandError::Failed("the export thread failed".to_owned())))
    })
}

/// One file from the plugin.
fn render(format: &str, request: &str) -> Result<(String, String), CommandError> {
    let plugins = crate::analyzer::plugins::bundled();
    let Some(entry) = plugins
        .report
        .loaded
        .iter()
        .find(|entry| entry.plugin.manifest().id == PLUGIN_ID)
    else {
        let why = plugins.refusal(PLUGIN_ID).map_or_else(
            || "plugins.error.not_available".to_owned(),
            |e| e.to_string(),
        );
        return Err(CommandError::Failed(format!(
            "the {PLUGIN_ID} plugin is not available: {why}"
        )));
    };
    let output = crate::analyzer::plugins::block_on(entry.plugin.render(format, request))
        .map_err(|error| CommandError::Failed(error.to_string()))?
        .map_err(|failure| {
            let params: Vec<String> = failure
                .params
                .iter()
                .map(|(key, value)| format!("{key}={value}"))
                .collect();
            CommandError::Failed(format!(
                "{} {}",
                failure.code().unwrap_or("plugins.error.failed"),
                params.join(" ")
            ))
        })?;
    Ok((output.file_name, output.text))
}

pub(super) fn store_failed(error: StoreError) -> CommandError {
    match error {
        StoreError::BadName(_) | StoreError::NotFound(_) => {
            CommandError::InvalidArgs(error.to_string())
        }
        other => CommandError::Failed(other.to_string()),
    }
}

fn exports_failed(error: ExportsError) -> CommandError {
    match error {
        ExportsError::BadName(_) => CommandError::InvalidArgs(error.to_string()),
        other => CommandError::Failed(other.to_string()),
    }
}

/// `path` relative to `root`, with `/`.
pub(super) fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

#[cfg(test)]
#[path = "export_tests.rs"]
mod tests;
