//! Engine comparison (RB-094-03): `compare_engines`, run by the host
//! (`crates/rbrowser/src/engine_compare/app.rs`). It acts on one view (`target`, the active view
//! when absent): its breakpoint and page, unless `breakpoint` and `url` name others. The page is
//! shot in Chromium (a hidden render view), Firefox and WebKit (a window of the engine's own)
//! at the breakpoint's size and DPR and each engine pair is diffed; the tool answers when the
//! run is done. Covers the command `compare.engines`.

use rmcp::{handler::server::wrapper::Parameters, schemars, tool, tool_router};
use serde::{Deserialize, Serialize};

use crate::RbMcpServer;
use crate::answer::{ToolResult, respond};
use crate::backend::ToolFailure;
use crate::target::{TargetArg, resolve_one};

/// The host tool behind it.
const COMPARE_TOOL: &str = "compare_engines";

/// `compare_engines`.
#[derive(Debug, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CompareEnginesParams {
    /// One view (a view id, or a breakpoint id shown once) whose breakpoint and page are
    /// compared; the active view when absent.
    #[serde(default, skip_serializing)]
    pub target: Option<TargetArg>,
    /// A breakpoint id over the target view's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub breakpoint: Option<String>,
    /// chromium, firefox, webkit; all three when absent.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub engines: Vec<String>,
    /// An http(s) or file URL over the target view's page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// The colour tolerance, 0 to 1 (over engineCompare.threshold and visual.threshold).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub threshold: Option<f64>,
    /// The share of differing pixels in percent up to which two engines count as the same,
    /// 0 to 100 (over engineCompare.maxDiffPercent).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_diff_percent: Option<f64>,
}

fn args_of(params: &impl Serialize) -> Result<serde_json::Value, ToolFailure> {
    serde_json::to_value(params).map_err(|error| ToolFailure::new("internal", error.to_string()))
}

#[tool_router(router = compare_engines_router, vis = "pub(crate)")]
impl RbMcpServer {
    #[tool(
        description = "Compare one page at one breakpoint in Chromium, Firefox and WebKit. target (one view; the active view when absent) gives the breakpoint and the page; optional breakpoint (id), engines (chromium, firefox, webkit; all three when absent), url (http, https or file), threshold (colour tolerance 0 to 1), maxDiffPercent (0 to 100). Each engine shoots the viewport at the breakpoint's size and DPR - Chromium in a hidden render view, Firefox and WebKit in a window of their own - with its own user agent, and every engine pair is diffed; anti-aliased pixels (font smoothing) never count. An engine that cannot run (not installed, switched off) is listed under skipped, not an error. Answers when done: runId, folder (.rbrowser/out/engine-compare/<runId>/), report (report.html: side by side, slider, difference), each engine's shot path, and per pair status (same, differs, sizeMismatch, failed), percent, differentPixels, the diff image's path and the differing areas (x, y, width, height in pixels). One comparison at a time."
    )]
    pub(crate) async fn compare_engines(
        &self,
        Parameters(params): Parameters<CompareEnginesParams>,
    ) -> ToolResult {
        let result = async {
            if params
                .threshold
                .is_some_and(|threshold| !(0.0..=1.0).contains(&threshold))
            {
                return Err(ToolFailure::new(
                    "invalid_arguments",
                    "threshold must be a number from 0 to 1",
                ));
            }
            if params
                .max_diff_percent
                .is_some_and(|percent| !(0.0..=100.0).contains(&percent))
            {
                return Err(ToolFailure::new(
                    "invalid_arguments",
                    "maxDiffPercent must be a number from 0 to 100",
                ));
            }
            let mut args = args_of(&params)?;
            if params.target.is_some() {
                let views = self.read(|backend| backend.views()).await?;
                if let Some(view) = resolve_one(params.target.as_ref(), &views)? {
                    args["view"] = serde_json::json!(view.0);
                }
            }
            let answer = self.backend.host_tool(COMPARE_TOOL, args).await?;
            Ok(self.backend.mask(&answer))
        }
        .await;
        respond(result)
    }
}

#[cfg(test)]
#[path = "compare_engines_tests.rs"]
mod tests;
