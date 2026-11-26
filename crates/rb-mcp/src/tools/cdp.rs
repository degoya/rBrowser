//! The CDP proxy: `cdp_endpoint` (RB-030-06). The answer carries the proxy's token, and the
//! token is full control of the views - `Runtime.evaluate`, the cookies, any navigation - past
//! every switch the MCP tools keep (`evaluate`, the URL policy, the request composer). So the tool
//! answers only while a person has switched on `mcp.allowEvaluate` as well as `cdp.enabled`;
//! otherwise `evaluate.disabled`, the token never asked for (RB-090-21, threat model T5).
//! Rotating the token is a person's decision and has no tool.

use rmcp::{handler::server::wrapper::Parameters, schemars, tool, tool_router};
use serde::Deserialize;

use crate::RbMcpServer;
use crate::answer::{ToolResult, respond};
use crate::backend::ToolFailure;
use crate::target::{TargetArg, resolve_one};
use crate::tools::page::{ALLOW_EVALUATE, EVALUATE_DISABLED};

/// `cdp_endpoint`.
#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CdpEndpointParams {
    /// One view: a view id or a breakpoint id shown once; it adds that view's page socket, none
    /// when absent.
    #[serde(default)]
    pub target: Option<TargetArg>,
}

#[tool_router(router = cdp_router, vis = "pub(crate)")]
impl RbMcpServer {
    #[tool(
        description = "The CDP endpoint for Playwright connectOverCDP(url, { headers }) and other CDP clients: url with its Authorization header, urlWithToken, the browser webSocketDebuggerUrl, with target that view's page socket, and wsl: the URL from WSL. Only the grid's views are targets; setViewportSize has no effect. Off unless a person switched on cdp.enabled (code cdp.disabled) and mcp.allowEvaluate (code evaluate.disabled): the endpoint is full control of the page."
    )]
    pub(crate) async fn cdp_endpoint(
        &self,
        Parameters(params): Parameters<CdpEndpointParams>,
    ) -> ToolResult {
        let result = async {
            if !self.backend.options().allow_evaluate {
                return Err(ToolFailure::new(
                    EVALUATE_DISABLED,
                    "the CDP endpoint is full control of the page and is given only while \
                     evaluate is on; a person can switch it on in rBrowser at Settings > \
                     Security & Privacy > Evaluate (rbrowser://settings/mcp.allowEvaluate)",
                )
                .with_setting(ALLOW_EVALUATE));
            }
            let view = match params.target {
                None => None,
                Some(target) => {
                    let views = self.read(|backend| backend.views()).await?;
                    resolve_one(Some(&target), &views)?
                }
            };
            self.backend.cdp_endpoint(view).await
        }
        .await;
        respond(result)
    }
}

#[cfg(test)]
#[path = "cdp_tests.rs"]
mod tests;
