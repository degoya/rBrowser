//! Playing a stored recording back (RB-090-66): `recorder.play` (`Leader F Y`) and the MCP tool
//! `recorder_play`. No export detour and nothing new in the flows folder: the recording becomes
//! a flow exactly as `recorder.export_flow` makes one - the `recorder-export` plugin's `flow`
//! ([`super::export::render_recording`]) - held in memory ([`InlineFlow`]), and runs through the
//! flows runner ([`crate::flows::app::start_inline`]) in the views of the recording's
//! breakpoints (every view when it names none), or in the views asked for. So it is a flow run
//! in everything else: the trust gate (a recording is project content, and its flow runs in
//! Node like any other), one run at a time, progress, the notices, `flows.cancel`,
//! `flows.rerun_last` and `.rbrowser/out/flows/<runId>/result.json`.
//!
//! - `recorder.play`: `{ recordingId?, views?, pace? }` - the last saved recording when
//!   `recordingId` is absent; `views` as `flows.run` takes them (view ids, breakpoint ids or
//!   labels); `pace` (`fast` or `recorded`) as `automation.recorder.playbackPace`, whose value
//!   it is when absent - `recorded` waits the pauses as they were recorded, each at most 10 s. The
//!   recording is read and rendered off the calling thread (a WSL project's disk is a network
//!   share, the plugin call blocks); what goes wrong before the run starts is a notice.
//! - `recorder_play` (a host tool): `{ recordingId?, views?, pace?, wait? }`, the result as `flow_run`
//!   answers it - per view passed, failed, skipped or timed out - or the `runId` at once with
//!   `wait: false`.

use std::sync::Arc;

use rb_commands::CommandError;
use rb_config::PlaybackPace;
use rb_engine::ViewHost as _;
use rb_ipc::{CoreToShell, Notification, NotificationLevel};
use rb_mcp::ToolFailure;
use rb_sync::recorder::Recording;
use serde::Deserialize;
use serde_json::{Value, json};

use super::export::{self, ExportFormat};
use super::glue;
use crate::control::Core;
use crate::flows::FlowError;
use crate::flows::app::{resolve_views, start_inline};
use crate::flows::run::InlineFlow;

/// `Leader F Y`: play the last (or the named) recording in its breakpoints' views.
pub const PLAY: &str = "recorder.play";
/// The MCP tool.
pub const RECORDER_PLAY_TOOL: &str = "recorder_play";
/// The notice when a recording could not be played before its run started; `{name}` and
/// `{error}`.
pub const FAILED_NOTICE: &str = "recorder.play.notice.failed";
/// The same for the last recording, when none was named; `{error}`.
pub const FAILED_LAST_NOTICE: &str = "recorder.play.notice.failed_last";

/// The arguments of `recorder.play` and `recorder_play`.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PlayArgs {
    /// The recording; the last saved one when absent.
    #[serde(default)]
    pub recording_id: Option<String>,
    /// The views: ids, breakpoint ids or labels; the recording's breakpoints when absent.
    #[serde(default)]
    pub views: Option<Value>,
    /// `fast` or `recorded`; `automation.recorder.playbackPace` when absent.
    #[serde(default)]
    pub pace: Option<PlaybackPace>,
    /// `recorder_play` only: wait for the result (the default).
    #[serde(default)]
    pub wait: Option<bool>,
}

/// `args` (an object or null) as [`PlayArgs`].
///
/// # Errors
///
/// [`CommandError::InvalidArgs`] for an unknown key or a wrong type.
pub fn args(args: &Value) -> Result<PlayArgs, CommandError> {
    if args.is_null() {
        return Ok(PlayArgs::default());
    }
    serde_json::from_value(args.clone())
        .map_err(|error| CommandError::InvalidArgs(error.to_string()))
}

/// The flow `recording` plays as: the plugin's flow among `files` (the `.ts` that is no
/// Playwright test), named `file` in results, run in the recording's breakpoints.
///
/// # Errors
///
/// [`CommandError::Failed`] when the plugin made no flow.
pub fn inline_flow(
    recording: &Recording,
    files: Vec<(String, String)>,
    file: String,
) -> Result<InlineFlow, CommandError> {
    let source = files
        .into_iter()
        .find(|(name, _)| name.ends_with(".ts") && !name.ends_with(".spec.ts"))
        .map(|(_, text)| text)
        .ok_or_else(|| {
            CommandError::Failed(format!(
                "the {} plugin made no flow of the recording",
                export::PLUGIN_ID
            ))
        })?;
    Ok(InlineFlow {
        source,
        file,
        views: recording
            .breakpoints
            .iter()
            .map(|breakpoint| breakpoint.id.clone())
            .collect(),
    })
}

/// The recording `id` (the last saved one when `None`) and the flow it plays as at `pace` (the
/// setting's when `None`). Reads the disk and waits for the plugin: never on the UI thread.
///
/// # Errors
///
/// No project, no such recording, or the plugin failed.
pub fn prepare(
    core: &Core,
    id: Option<String>,
    pace: Option<PlaybackPace>,
) -> Result<(String, InlineFlow), CommandError> {
    let root = core
        .host()
        .project_root()
        .ok_or_else(|| CommandError::Failed("no project is open".to_owned()))?;
    let store =
        glue::store_of(core.host()).map_err(|error| CommandError::Failed(error.to_string()))?;
    let id = match id {
        Some(id) => id,
        None => export::latest(&store)?.ok_or_else(|| {
            CommandError::InvalidArgs(
                "no recording to play yet: record one, or pass `recordingId`".to_owned(),
            )
        })?,
    };
    let recording = store.load(&id).map_err(export::store_failed)?;
    let file = store.path_of(&id).map_or_else(
        |_| format!("{id}.json"),
        |path| export::relative(&root, &path),
    );
    let files = export::render_recording_at(core, &recording, None, ExportFormat::Flow, pace)?;
    let flow = inline_flow(&recording, files, file)?;
    Ok((id, flow))
}

/// `recorder.play`: checks the arguments and the views, then reads, renders and starts the
/// recording on a blocking thread of the core's runtime. Answers at once; the run's start and
/// end are the flows' notices, a failure before it starts is [`FAILED_NOTICE`].
///
/// # Errors
///
/// Bad arguments, an unknown view, or a closed window.
pub fn run(core: &Core, raw: &Value) -> Result<Value, CommandError> {
    let PlayArgs {
        recording_id,
        views,
        pace,
        ..
    } = args(raw)?;
    let core = core
        .arc()
        .ok_or_else(|| CommandError::Failed("the window is closed".to_owned()))?;
    let views = resolve_views(&core.host().views(), views.as_ref().unwrap_or(&Value::Null))
        .map_err(|error| CommandError::InvalidArgs(error.to_string()))?;
    let asked = recording_id.clone();
    let asked_for = recording_id.clone();
    let runtime = core.runtime().clone();
    runtime.spawn_blocking(move || {
        let failed = match prepare(&core, recording_id, pace) {
            // An untrusted workspace has the runner's own notice, with the way to trust it.
            Ok((name, flow)) => match start_inline(&core, &name, flow, views) {
                Ok(_) | Err(FlowError::Untrusted) => None,
                Err(error) => Some(error.to_string()),
            },
            Err(error) => Some(error.to_string()),
        };
        if let Some(error) = failed {
            core.notify(failed_notice(asked_for.as_deref(), &error));
        }
    });
    Ok(json!({ "recordingId": asked }))
}

/// `recorder_play`: the recording's run, its result once it ends (`wait`, the default) or its
/// `runId` at once.
///
/// # Errors
///
/// The command's refusals, and the flows' with their codes (`flows.untrusted`, `flows.busy`).
pub async fn tool(core: &Arc<Core>, raw: Value) -> Result<Value, ToolFailure> {
    let PlayArgs {
        recording_id,
        views,
        pace,
        wait,
    } = args(&raw).map_err(ToolFailure::from)?;
    let views = resolve_views(&core.host().views(), views.as_ref().unwrap_or(&Value::Null))
        .map_err(|error| flow_failure(&error))?;
    let blocking = Arc::clone(core);
    let (name, flow) = tokio::task::spawn_blocking(move || prepare(&blocking, recording_id, pace))
        .await
        .map_err(|_| ToolFailure::new("internal", "the recording could not be read"))?
        .map_err(ToolFailure::from)?;
    let started = start_inline(core, &name, flow, views).map_err(|error| flow_failure(&error))?;
    if !wait.unwrap_or(true) {
        return Ok(json!({ "recordingId": name, "runId": started.run_id }));
    }
    match started.done.await {
        Ok(Ok(result)) => serde_json::to_value(result)
            .map_err(|error| ToolFailure::new("internal", error.to_string())),
        Ok(Err(error)) => Err(flow_failure(&error)),
        Err(_) => Err(ToolFailure::new(
            "flows.ended",
            "the run ended without a result (the window closed)",
        )),
    }
}

/// A flow's refusal as a tool failure: its code, message and setting.
pub fn flow_failure(error: &FlowError) -> ToolFailure {
    let failure = ToolFailure::new(error.code(), error.to_string());
    match error.settings_key() {
        Some(key) => failure.with_setting(key),
        None => failure,
    }
}

/// The notice of a play that did not start: the recording asked for, else the last.
#[must_use]
pub fn failed_notice(name: Option<&str>, error: &str) -> CoreToShell {
    let mut args = vec![("error".to_owned(), rb_diag::redact(error))];
    if let Some(name) = name {
        args.push(("name".to_owned(), name.to_owned()));
    }
    CoreToShell::Notification(Notification {
        level: NotificationLevel::Error,
        message_key: if name.is_some() {
            FAILED_NOTICE
        } else {
            FAILED_LAST_NOTICE
        }
        .to_owned(),
        args: args.into_iter().collect(),
        settings_key: None,
    })
}

#[cfg(test)]
#[path = "play_tests.rs"]
mod tests;
