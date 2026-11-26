//! Captures, checks and sessions: `screenshot` (RB-050-03, run by the host), `checks_run`
//! (RB-060-01, the command `checks.run_all`), `checks_cancel` (RB-080-89, the command
//! `checks.cancel`), `session_assign` and `session_clear` (RB-020-07, the commands
//! `session.assign` and `session.clear`).

use rmcp::{handler::server::wrapper::Parameters, schemars, tool, tool_router};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::NoArgs;
use super::forward::Answers;
use crate::RbMcpServer;
use crate::answer::{ToolResult, respond};
use crate::backend::ToolFailure;
use crate::run_target::{RunTarget, Split, with_scope};
use crate::target::TargetArg;

/// What `screenshot` captures of each view.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum CaptureMode {
    /// The viewport at the breakpoint's size.
    Viewport,
    /// The whole page.
    FullPage,
    /// One element (`selector`).
    Element,
}

/// `screenshot`.
#[derive(Debug, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScreenshotParams {
    /// `"all"`, a view id or a breakpoint id; the active view when absent.
    #[serde(default, skip_serializing)]
    pub target: Option<TargetArg>,
    /// `viewport` (default), `fullPage` or `element`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<CaptureMode>,
    /// The element's CSS selector (mode `element`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selector: Option<String>,
    /// CSS px around the element; the setting `capture.elementPadding` when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub padding: Option<u32>,
    /// One overview image of the views side by side instead of a file each.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overview: Option<bool>,
    /// Wait this long first, at most 60000 ms.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delay_ms: Option<u64>,
    /// Write the files (default) and answer their paths, or answer the PNGs as base64.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub save: Option<bool>,
    /// Hide cookie banners and chat widgets; the setting capture.hideBanners when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hide_banners: Option<bool>,
    /// Freeze animations and videos; the setting capture.freezeAnimations when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub freeze_animations: Option<bool>,
    /// The element at this point instead of a selector (mode `element`, one view): CSS px of
    /// the page from the viewport's top-left corner (RB-080-43).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<PointParam>,
}

/// A point of a view's page in CSS px.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PointParam {
    pub x: u32,
    pub y: u32,
}

/// `checks_run`.
#[derive(Debug, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChecksRunParams {
    /// `"all"` (default), a view id or a breakpoint id. Also `"visible"` (the views the grid shows)
    /// or a list of view ids and breakpoint ids.
    #[serde(default, skip_serializing)]
    pub target: Option<RunTarget>,
    /// Only these check plugins (ids); every enabled one when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugins: Option<Vec<String>>,
    /// The page to check, http(s); each view's own when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

/// `session_assign`.
#[derive(Debug, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SessionAssignParams {
    /// `"all"`, a view id or a breakpoint id; the active view when absent.
    #[serde(default, skip_serializing)]
    pub target: Option<TargetArg>,
    /// The session: `default` or one of the project's `sessions`.
    pub session: String,
}

/// `session_clear`.
#[derive(Debug, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SessionClearParams {
    /// The session whose cookies and storage go.
    pub session: String,
}

fn args_of(params: &impl Serialize) -> Result<Value, ToolFailure> {
    serde_json::to_value(params)
        .map_err(|error| ToolFailure::new("mcp.internal", error.to_string()))
}

#[tool_router(router = capture_router, vis = "pub(crate)")]
impl RbMcpServer {
    #[tool(
        description = "Capture views as PNG: mode viewport (the breakpoint's size), full page (scrolled through first so lazy images load, then back; stacked sections, at most 65535 px and 256 megapixels; a longer page is cut and says truncated: true with pageHeight; images still loading after 4 s say imagesPending: true) or one element (selector, or `at` {x, y} in CSS px of the page; one view; with padding). overview: one image of all views side by side. delayMs waits first (at most 60000). save (default true) writes under the project's screenshot folder and answers the paths; save false answers base64 PNGs, at most mcp.maxAnswerBytes together (answer.too_large). hideBanners and freezeAnimations override the capture.* settings."
    )]
    pub(crate) async fn screenshot(
        &self,
        Parameters(params): Parameters<ScreenshotParams>,
    ) -> ToolResult {
        let result = match args_of(&params) {
            // Inline PNGs of every view together stay within mcp.maxAnswerBytes.
            Ok(args) if params.save == Some(false) => {
                let limit = self.backend.options().max_answer_bytes;
                self.host_per_target_inline("screenshot", params.target.as_ref(), args, limit)
                    .await
            }
            Ok(args) => {
                self.host_per_target("screenshot", params.target.as_ref(), args, Answers::Joined)
                    .await
            }
            Err(failure) => Err(failure),
        };
        respond(result)
    }

    #[tool(
        description = "Run every enabled check plugin (SEO, accessibility, console, ...) over the targeted views; plugins limits it to those plugin ids, url checks that page instead of the views' own. Answers the views checked (breakpoints no view shows as notShown), the merged findings - deduplicated across views and plugins, each with a stable id -, a summary per severity and plugin, and jobs (each plugin x view: done with its count, failed with its code, skipped with the reason). checks_cancel stops a run (cancelled: true). Also written to .rbrowser/out/checks/."
    )]
    pub(crate) async fn checks_run(
        &self,
        Parameters(params): Parameters<ChecksRunParams>,
    ) -> ToolResult {
        let result = async {
            let args = args_of(&params)?;
            match self.split_target(params.target.as_ref()).await? {
                Split::Target(target) => {
                    let target = target.unwrap_or_else(TargetArg::all);
                    self.per_target("checks.run_all", Some(&target), args).await
                }
                // RB-099-10: one run over the views of the scope's breakpoints.
                Split::Scope(scope) => {
                    self.command("checks.run_all", with_scope(&args, &scope))
                        .await
                }
            }
        }
        .await;
        respond(result)
    }

    #[tool(
        description = "Cancel the check run in progress: jobs not started are skipped, running ones dropped; the findings of finished jobs stay and are the run's answer, with cancelled: true. Answers cancelled: false when no run is in progress."
    )]
    pub(crate) async fn checks_cancel(&self, _args: Parameters<NoArgs>) -> ToolResult {
        respond(self.command("checks.cancel", Value::Null).await)
    }

    #[tool(
        description = "Move the targeted views into a session (\"default\" or one of the project's sessions): each view is created anew in the session's own cookie and storage context and loads its URL again. Answers the views moved (from, view)."
    )]
    pub(crate) async fn session_assign(
        &self,
        Parameters(params): Parameters<SessionAssignParams>,
    ) -> ToolResult {
        let result = match args_of(&params) {
            Ok(args) => {
                self.per_target("session.assign", params.target.as_ref(), args)
                    .await
            }
            Err(failure) => Err(failure),
        };
        respond(result)
    }

    #[tool(
        description = "Clear a session's data: every cookie of the session, and the storage (local and session storage, IndexedDB, cache storage, service workers) of every origin its views show; the session's views reload. Other sessions keep theirs."
    )]
    pub(crate) async fn session_clear(
        &self,
        Parameters(params): Parameters<SessionClearParams>,
    ) -> ToolResult {
        respond(
            self.command("session.clear", json!({ "session": params.session }))
                .await,
        )
    }
}

#[cfg(test)]
#[path = "capture_tests.rs"]
mod tests;
