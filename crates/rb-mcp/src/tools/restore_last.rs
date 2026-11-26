//! The pages of the last session (RB-092-04): `views_restore_last`, the command
//! `views.restore_last` once per resolved target. The answer names views, never a URL: the
//! kept pages may carry tokens.

use rmcp::{handler::server::wrapper::Parameters, schemars, tool, tool_router};
use serde::Deserialize;
use serde_json::json;

use crate::RbMcpServer;
use crate::answer::{ToolResult, respond};
use crate::target::TargetArg;

/// The command behind `views_restore_last`.
const RESTORE_LAST: &str = "views.restore_last";

/// `views_restore_last`.
#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RestoreLastParams {
    /// `"all"` (default), a view id or a breakpoint id.
    #[serde(default)]
    pub target: Option<TargetArg>,
}

#[tool_router(router = restore_last_router, vis = "pub(crate)")]
impl RbMcpServer {
    #[tool(
        description = "Open in the targeted views the pages they showed when rBrowser last closed (kept per view in the browser profile, never in the project): a page under an environment opens under the environment in effect now; a decoupled view is decoupled again; a view with nothing kept stays. Answers views, decoupled and stored (false without a last session); never a URL."
    )]
    pub(crate) async fn views_restore_last(
        &self,
        Parameters(params): Parameters<RestoreLastParams>,
    ) -> ToolResult {
        respond(
            self.per_target(RESTORE_LAST, params.target.as_ref(), json!({}))
                .await,
        )
    }
}

#[cfg(test)]
#[path = "restore_last_tests.rs"]
mod tests;
