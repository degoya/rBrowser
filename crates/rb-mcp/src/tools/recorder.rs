//! The recorder (RB-050-01, RB-050-02): `recorder_start` and `recorder_stop`, the commands
//! `recorder.start`, `recorder.stop` and `recorder.pause`; `recorder_export`, run by the host (no
//! single command writes both formats). The recorder belongs to the window: only the start
//! names a view, the one it records from. No answer carries a recorded value in clear - the
//! recording keeps secrets as `{{secret:<name>}}` - and the export's answer passes the host's
//! mask on top.

use rmcp::{handler::server::wrapper::Parameters, schemars, tool, tool_router};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::object;
use crate::RbMcpServer;
use crate::answer::{ToolResult, respond};
use crate::backend::ToolFailure;
use crate::target::{TargetArg, resolve_one};

/// The host tool behind `recorder_export`.
const EXPORT_TOOL: &str = "recorder_export";

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

#[tool_router(router = recorder_router, vis = "pub(crate)")]
impl RbMcpServer {
    #[tool(
        description = "Start recording one view's interactions (clicks, typing, keys, form changes, navigations) as a cleaned flow, or resume a paused recording. `target`: one view (a view id or a breakpoint id shown once) that becomes the active view first; the active view when absent. `name` ([A-Za-z0-9_-], at most 64 characters) names the file; a timestamped name when absent. Password fields and the configured secret fields are recorded as {{secret:<name>}}, never in clear. Answers the recorder's state, the name and the step count."
    )]
    pub(crate) async fn recorder_start(
        &self,
        Parameters(params): Parameters<RecorderStartParams>,
    ) -> ToolResult {
        let result = async {
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
}

#[cfg(test)]
#[path = "recorder_tests.rs"]
mod tests;
