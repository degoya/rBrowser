//! Environment comparison (RB-080-03): `env_compare`, `env_compare_result`, `env_compare_live`
//! and `env_compare_cancel`, run by the host (`crates/rbrowser/src/compare/app.rs`). `env_compare`
//! takes `target` ("all" or views): the views' breakpoints are compared (unless `breakpoints`
//! names them) and the first one's page (unless `paths` names them); the shots are taken in
//! hidden render views of their own, never in the grid, and the run goes on in the background.
//! `env_compare_live` acts on one view: its breakpoint, once per environment, side by side. The
//! commands `env.compare.run`, `env.compare.report`, `env.compare.live` and `env.compare.cancel`
//! are covered by them.

use rmcp::{handler::server::wrapper::Parameters, schemars, tool, tool_router};
use serde::{Deserialize, Serialize};

use crate::RbMcpServer;
use crate::answer::{ToolResult, respond};
use crate::backend::ToolFailure;
use crate::target::{TargetArg, resolve, resolve_one};

/// The host tools behind the three tools.
const COMPARE_TOOL: &str = "env_compare";
const RESULT_TOOL: &str = "env_compare_result";
const LIVE_TOOL: &str = "env_compare_live";
const CANCEL_TOOL: &str = "env_compare_cancel";

/// `env_compare`.
#[derive(Debug, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EnvCompareParams {
    /// `"all"`, a view id or a breakpoint id: the views whose breakpoints (and the first one's
    /// page) are compared; the active view's page and the setting envCompare.breakpoints when
    /// absent.
    #[serde(default, skip_serializing)]
    pub target: Option<TargetArg>,
    /// Environment ids (at least two); envCompare.environments, else every environment, when
    /// absent.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub environments: Vec<String>,
    /// Paths (`/about`) or URLs under an environment; the target view's page when absent.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub paths: Vec<String>,
    /// Breakpoint ids; over the target views' breakpoints.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub breakpoints: Vec<String>,
    /// Capture the whole page (over envCompare.fullPage).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub full_page: Option<bool>,
    /// The colour tolerance, 0 to 1 (over envCompare.threshold and visual.threshold).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub threshold: Option<f64>,
    /// Hide cookie banners and chat widgets; the setting capture.hideBanners when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hide_banners: Option<bool>,
    /// Freeze animations and videos; the setting capture.freezeAnimations when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub freeze_animations: Option<bool>,
}

/// `env_compare_result` and `env_compare_cancel`.
#[derive(Debug, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EnvCompareResultParams {
    /// The run (`env_compare` answers it); the latest (for a cancel: the running one) when
    /// absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
}

/// `env_compare_live`.
#[derive(Debug, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EnvCompareLiveParams {
    /// One view (a view id, or a breakpoint id shown once) whose breakpoint and page are
    /// compared; the active view when absent.
    #[serde(default, skip_serializing)]
    pub target: Option<TargetArg>,
    /// Environment ids; envCompare.environments, else every environment, when absent.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub environments: Vec<String>,
    /// true shows the comparison, false leaves it; toggles when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
}

fn args_of(params: &impl Serialize) -> Result<serde_json::Value, ToolFailure> {
    serde_json::to_value(params).map_err(|error| ToolFailure::new("internal", error.to_string()))
}

#[tool_router(router = env_compare_router, vis = "pub(crate)")]
impl RbMcpServer {
    #[tool(
        description = "Compare the same pages in two or more environments (local, staging, prod) pixel by pixel. target (\"all\" or views; the active view's page when absent) picks the breakpoints and the page; optional environments (ids, at least two), paths (/about or a URL under an environment; a foreign host is refused), breakpoints, fullPage, threshold (0 to 1). Each path is captured in every environment and breakpoint in hidden render views (the grid stays usable) and every environment pair is diffed. Answers at once with runId, the folder .rbrowser/out/env-compare/<runId>/ (PNGs, diff images, report.json, report.html) and the shots and diffs; follow it with env_compare_result. One run at a time. hideBanners and freezeAnimations override capture.hideBanners and capture.freezeAnimations (banners hidden, animations frozen before each capture, undone after)."
    )]
    pub(crate) async fn env_compare(
        &self,
        Parameters(params): Parameters<EnvCompareParams>,
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
            let mut args = args_of(&params)?;
            if let Some(target) = params.target.clone() {
                let views = self.read(|backend| backend.views()).await?;
                let ids: Vec<u32> = resolve(Some(&target), &views)?
                    .view_ids(&views)
                    .into_iter()
                    .map(|id| id.0)
                    .collect();
                args["views"] = serde_json::json!(ids);
            }
            let answer = self.backend.host_tool(COMPARE_TOOL, args).await?;
            Ok(self.backend.mask(&answer))
        }
        .await;
        respond(result)
    }

    #[tool(
        description = "The state of an environment comparison (runId; the latest when absent): running or finished, the shots taken and failed, the pairs that differ, the folder and report.html once written, and per path, environment pair and breakpoint the status (same, differs, sizeMismatch, failed, pending), the share of differing pixels in percent and the diff image."
    )]
    pub(crate) async fn env_compare_result(
        &self,
        Parameters(params): Parameters<EnvCompareResultParams>,
    ) -> ToolResult {
        let result = async {
            let args = args_of(&params)?;
            let answer = self.backend.host_tool(RESULT_TOOL, args).await?;
            Ok(self.backend.mask(&answer))
        }
        .await;
        respond(result)
    }

    #[tool(
        description = "Show one view's breakpoint once per environment side by side (target: one view; the active view when absent), each on the page the view shows under its own environment, synced: a click on an internal link moves every view to the same path under its own host, a link to another host is not rewritten. Optional environments (ids), enabled (false returns to the grid before; toggles when absent). Answers whether it shows and each view's environment and URL."
    )]
    pub(crate) async fn env_compare_live(
        &self,
        Parameters(params): Parameters<EnvCompareLiveParams>,
    ) -> ToolResult {
        let result = async {
            let mut args = args_of(&params)?;
            if params.target.is_some() {
                let views = self.read(|backend| backend.views()).await?;
                if let Some(view) = resolve_one(params.target.as_ref(), &views)? {
                    args["view"] = serde_json::json!(view.0);
                }
            }
            let answer = self.backend.host_tool(LIVE_TOOL, args).await?;
            Ok(self.backend.mask(&answer))
        }
        .await;
        respond(result)
    }

    #[tool(
        description = "Cancel a running environment comparison (runId; the running one when absent). The shots taken stay, report.json and report.html are written for what was compared, and the run's state becomes cancelled. Answers the runId and whether it was still running."
    )]
    pub(crate) async fn env_compare_cancel(
        &self,
        Parameters(params): Parameters<EnvCompareResultParams>,
    ) -> ToolResult {
        let result = async {
            let args = args_of(&params)?;
            let answer = self.backend.host_tool(CANCEL_TOOL, args).await?;
            Ok(self.backend.mask(&answer))
        }
        .await;
        respond(result)
    }
}

#[cfg(test)]
#[path = "env_compare_tests.rs"]
mod tests;
