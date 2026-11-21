//! The recorder (RB-050-01, RB-050-02): `recorder_start` and `recorder_stop`, the commands
//! `recorder.start`, `recorder.stop` and `recorder.pause`; `recorder_export`, run by the host (no
//! single command writes both formats); `recorder_play` (RB-090-66), run by the host too, plays a
//! stored recording through the flows runner as `flow_run` runs a flow. The recorder belongs to
//! the window: the start names the one view it records from, the playback the views it plays in. No answer carries a recorded value in clear - the
//! recording keeps secrets as `{{secret:<name>}}` - and the export's answer passes the host's
//! mask on top.

use rmcp::{handler::server::wrapper::Parameters, schemars, tool, tool_router};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::object;
use crate::RbMcpServer;
use crate::answer::{ToolResult, respond};
use crate::backend::ToolFailure;
use crate::target::{Resolved, TargetArg, resolve, resolve_one};

/// The host tool behind `recorder_export`.
const EXPORT_TOOL: &str = "recorder_export";
/// The host tool behind `recorder_play`.
const PLAY_TOOL: &str = "recorder_play";

/// `recorder_start`.
#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RecorderStartParams {
    /// One view to record from (a view id or a breakpoint id shown once): it becomes the active
    /// view first. The active view when absent.
    #[serde(default)]
    pub target: Option<TargetArg>,
    /// The recording's name (`[A-Za-z0-9_-]`, at most 64 characters); a timestamped one when
    /// absent.
    #[serde(default)]
    pub name: Option<String>,
    /// Continue a stored recording from one of its steps (RB-092-01) instead of starting a new
    /// one: its steps up to there play first, at the pace `fast`, then the new steps land after
    /// it. `target` and `name` do not apply.
    #[serde(default)]
    pub continue_from: Option<ContinueFrom>,
}

/// Where a recording continues (RB-092-01).
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContinueFrom {
    /// The stored recording.
    pub recording_id: String,
    /// The step after which the new steps land.
    pub step_id: u32,
}

/// `recorder_stop`.
#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RecorderStopParams {
    /// Pause (or resume) instead of stopping.
    #[serde(default)]
    pub pause: bool,
}

/// What `recorder_export` writes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    /// The rBrowser flow.
    Flow,
    /// The Playwright test (and its config, per `automation.recorderExport.includeConfig`).
    Spec,
    /// Both.
    Both,
}

/// `recorder_export`.
#[derive(Debug, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RecorderExportParams {
    /// The recording (`recorder_stop` answers its name); the last saved one when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recording_id: Option<String>,
    /// `flow`, `spec` or `both` (the default).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<ExportFormat>,
    /// The export's name (`[A-Za-z0-9_-]`, at most 64 characters); the recording's when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// How `recorder_play` paces the steps (`automation.recorder.playbackPace`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum PlaybackPace {
    /// Back to back, each step waiting only for its element.
    Fast,
    /// The pause before each step as recorded, at most 10 s.
    Recorded,
}

/// `recorder_play`.
#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RecorderPlayParams {
    /// The recording (`recorder_stop` answers its name); the last saved one when absent.
    #[serde(default)]
    pub recording_id: Option<String>,
    /// `"all"`, a view id or a breakpoint id; when absent, the views of the breakpoints the
    /// recording was made in, else all.
    #[serde(default)]
    pub target: Option<TargetArg>,
    /// `fast` or `recorded`; `automation.recorder.playbackPace` when absent.
    #[serde(default)]
    pub pace: Option<PlaybackPace>,
    /// Wait for the result (the default); `false` answers the `runId` at once and the result
    /// lands in `.rbrowser/out/flows/<runId>/result.json` (`flow_list` shows the run going on).
    #[serde(default)]
    pub wait: Option<bool>,
    /// Play from this step on (RB-092-01, "Play from here").
    #[serde(default)]
    pub from_step: Option<u32>,
}

#[tool_router(router = recorder_router, vis = "pub(crate)")]
impl RbMcpServer {
    #[tool(
        description = "Start recording one view's interactions (clicks, typing, keys, form changes, navigations) as a cleaned flow, or resume a paused recording. `target`: one view (a view id or a breakpoint id shown once) that becomes the active view first; the active view when absent. `name` ([A-Za-z0-9_-], at most 64 characters) names the file; a timestamped name when absent. Password fields and the configured secret fields are recorded as {{secret:<name>}}, never in clear. Refused while a flow runs (recorder.flow_running): its navigations would become steps. continueFrom { recordingId, stepId } continues a stored recording from that step instead: its steps up to there play first at the pace fast (a failure there starts no recording and the notice says why), then the new steps land after it and the stop writes them into that recording. Answers the recorder's state, the name and the step count."
    )]
    pub(crate) async fn recorder_start(
        &self,
        Parameters(params): Parameters<RecorderStartParams>,
    ) -> ToolResult {
        let result = async {
            if let Some(from) = params.continue_from {
                let args = json!({ "recordingId": from.recording_id, "stepId": from.step_id });
                return self.command("recorder.continue_from", args).await;
            }
            let views = self.read(|backend| backend.views()).await?;
            let view = resolve_one(params.target.as_ref(), &views)?;
            let args = object([
                ("name", params.name.map(Value::String)),
                ("target", view.map(|view| json!(view.0))),
            ]);
            self.command("recorder.start", args).await
        }
        .await;
        respond(result)
    }

    #[tool(
        description = "Stop the recording and write it to the project's recordings folder (automation.recorder.dir, default .rbrowser/out/recordings/<name>.json); an existing file is never overwritten - the next free <name>-<n> is taken. With pause: true the recording only pauses, or resumes when paused. Answers the state, the name (the recordingId of recorder_export), the step count and, after a stop, the file relative to the project root."
    )]
    pub(crate) async fn recorder_stop(
        &self,
        Parameters(params): Parameters<RecorderStopParams>,
    ) -> ToolResult {
        let id = if params.pause {
            "recorder.pause"
        } else {
            "recorder.stop"
        };
        respond(self.command(id, json!({})).await)
    }

    #[tool(
        description = "Export a stored recording as an rBrowser flow (format \"flow\", <name>.ts with page, view and expect injected), as a Playwright test (\"spec\", <name>.spec.ts plus playwright.config.ts with one project per breakpoint) or both (\"both\", the default). recordingId names the recording (recorder_stop answers it); the last saved one when absent. name renames the export. The files go into a new folder of automation.recorderExport.dir (default .rbrowser/out/exports/<name>/); nothing is overwritten. Secrets stay placeholders: the code reads process.env.RB_SECRET_<NAME>, and the answer is masked like the log. Answers the folder and each file's path and content."
    )]
    pub(crate) async fn recorder_export(
        &self,
        Parameters(params): Parameters<RecorderExportParams>,
    ) -> ToolResult {
        let result = async {
            let args = serde_json::to_value(&params)
                .map_err(|error| ToolFailure::new("internal", error.to_string()))?;
            let answer = self.backend.host_tool(EXPORT_TOOL, args).await?;
            Ok(self.backend.mask(&answer))
        }
        .await;
        respond(result)
    }

    #[tool(
        description = "Play a stored recording back in every chosen view at once, without exporting it: the recording becomes the same flow recorder_export writes (format \"flow\") and runs through the flows runner, held in memory - nothing is written to the flows folder. recordingId names the recording (recorder_stop answers it); the last saved one when absent. pace \"recorded\" waits the pause before each step as recorded (at most 10 s), \"fast\" runs them back to back; automation.recorder.playbackPace when absent. Answers per view passed, failed, skipped (Firefox and WebKit views, with the reason) or timedOut, with duration, steps, the error with its step and line, and a screenshot file on failure - the result.json of a flow run. Refused with flows.untrusted until a person trusts the workspace; one run at a time (flows.busy), none while a recording takes steps (flows.recording)."
    )]
    pub(crate) async fn recorder_play(
        &self,
        Parameters(params): Parameters<RecorderPlayParams>,
    ) -> ToolResult {
        let result = async {
            let views: Option<Vec<u32>> = match params.target {
                None => None,
                Some(target) => {
                    let views = self.read(|backend| backend.views()).await?;
                    match resolve(Some(&target), &views)? {
                        Resolved::All => Some(views.iter().map(|view| view.id.0).collect()),
                        Resolved::Views(ids) => Some(ids.iter().map(|id| id.0).collect()),
                    }
                }
            };
            let args = object([
                ("recordingId", params.recording_id.map(Value::String)),
                ("views", views.map(|ids| json!(ids))),
                ("pace", params.pace.map(|pace| json!(pace))),
                ("wait", params.wait.map(Value::Bool)),
                ("fromStep", params.from_step.map(|step| json!(step))),
            ]);
            let answer = self.backend.host_tool(PLAY_TOOL, args).await?;
            Ok(self.backend.mask(&answer))
        }
        .await;
        respond(result)
    }
}

#[cfg(test)]
#[path = "recorder_tests.rs"]
mod tests;
