//! Web Vitals and resources (RB-060-10): `view_metrics`, a read the host answers
//! (`Backend::host_tool`) from what it keeps per view. The display switches `debug.vitals` and
//! `debug.resources` have no tool; this is their data.

use rmcp::{handler::server::wrapper::Parameters, schemars, tool, tool_router};
use serde::Deserialize;
use serde_json::json;

use crate::RbMcpServer;
use crate::answer::{ToolResult, respond};
use crate::target::{TargetArg, resolve};

/// The host tool behind `view_metrics`.
pub const VIEW_METRICS_TOOL: &str = "view_metrics";

/// `view_metrics`.
#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ViewMetricsParams {
    /// `"all"` (default), a view id or a breakpoint id.
    #[serde(default)]
    pub target: Option<TargetArg>,
}

#[tool_router(router = metrics_router, vis = "pub(crate)")]
impl RbMcpServer {
    #[tool(
        description = "Read the Web Vitals and resources of the targeted views (target: \"all\" (default), a view id or a breakpoint id). Per view: vitals lcp, cls, inp, fcp and ttfb, each with value (ms; CLS unitless), rating (\"good\", \"needs-improvement\" or \"poor\" by web.dev's thresholds, overridable in vitals.thresholds) and the element behind it (lcp element, cls sources, inp target as CSS paths); resources cpuPercent (the page's main-thread share), memoryMb (its JS heap), cpuWarn and ramWarn (resources.cpuWarnPercent, resources.ramWarnMb); throttle (visible, hidden or cpuLimited); segment and soft (a soft navigation measures only CLS and INP). A missing value is {na, reason}: pending (not measured yet, e.g. INP before an interaction), unsupported or engine (the engine cannot give it; Firefox and WebKit views have no resources), soft, or off with the settingsKey that switched it off."
    )]
    pub(crate) async fn view_metrics(
        &self,
        Parameters(params): Parameters<ViewMetricsParams>,
    ) -> ToolResult {
        let result = async {
            let views = self.read(|backend| backend.views()).await?;
            let ids: Vec<u32> = resolve(params.target.as_ref(), &views)?
                .view_ids(&views)
                .into_iter()
                .map(|view| view.0)
                .collect();
            let answer = self
                .backend
                .host_tool(VIEW_METRICS_TOOL, json!({ "views": ids }))
                .await?;
            Ok(self.backend.mask(&answer))
        }
        .await;
        respond(result)
    }
}

#[cfg(test)]
#[path = "metrics_tests.rs"]
mod tests;
