//! The CDP proxy: `cdp_endpoint` (RB-030-06). The answer carries the proxy's token, and the
//! token is full control of the views - `Runtime.evaluate`, the cookies, any navigation - past
//! every switch the MCP tools keep (`evaluate`, the URL policy, the request composer). So the tool
//! answers only while a person has switched on `mcp.allowEvaluate` as well as `cdp.enabled`;
//! otherwise `evaluate_disabled`, the token never asked for (RB-090-21, threat model T5).
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
    /// One view (id or breakpoint id): its page socket is added.
    #[serde(default)]
    pub target: Option<TargetArg>,
}

#[tool_router(router = cdp_router, vis = "pub(crate)")]
impl RbMcpServer {
    #[tool(
        description = "The CDP endpoint for Playwright connectOverCDP(url, { headers }) and other CDP clients: url with its Authorization header, urlWithToken for clients without headers, the browser webSocketDebuggerUrl, with target also the page socket of that view, and wsl: the URL from WSL (Windows' own in mirrored mode, a tunnel into the distribution in NAT mode). Only the grid's views are targets. rBrowser owns each view's size: setViewportSize has no effect. Off unless a person switched the proxy on (cdp.enabled, code cdp.disabled) and allowed page JavaScript over MCP (mcp.allowEvaluate, code evaluate_disabled): the endpoint is full control of the page."
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
