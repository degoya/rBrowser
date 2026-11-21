//! The presentation over the internet (RB-096-05): `portal_present_status` and
//! `portal_present_stop`, run by the host (`crates/rbrowser/src/portal/present/mcp.rs`). No tool
//! starts a presentation or issues a viewer link: sharing the site on the internet is a person's
//! act (`portal.present.start`, coverage `omitted`), and the link with its one-time token never
//! reaches an MCP answer - nor does the tunnel's public address. The tools act on the
//! presentation, not on views: none takes `target`.

use rmcp::{handler::server::wrapper::Parameters, tool, tool_router};
use serde_json::json;

use super::portal::PortalParams;
use crate::RbMcpServer;
use crate::answer::{ToolResult, respond};

/// `portal_present_stop`.
pub const STOP_TOOL: &str = "portal_present_stop";
/// `portal_present_status`.
pub const STATUS_TOOL: &str = "portal_present_status";

#[tool_router(router = portal_present_router, vis = "pub(crate)")]
impl RbMcpServer {
    #[tool(
        description = "End the portal's presentation over the internet: every viewer is disconnected at once and the Cloudflare tunnel closes. Answers the state after it. MCP cannot start a presentation: a person starts it in rBrowser (Ctrl+Shift+Space S W) and hands out the one-time link, which never reaches MCP."
    )]
    pub(crate) async fn portal_present_stop(
        &self,
        Parameters(_): Parameters<PortalParams>,
    ) -> ToolResult {
        respond(self.backend.host_tool(STOP_TOOL, json!({})).await)
    }

    #[tool(
        description = "The state of the portal's presentation over the internet: running, phase (off, downloading, starting, live), viewers and maxViewers, linkOpen and linkExpiresInSec (a viewer link works once), startedAtMs. Never the link, its token or the public address."
    )]
    pub(crate) async fn portal_present_status(
        &self,
        Parameters(_): Parameters<PortalParams>,
    ) -> ToolResult {
        respond(self.backend.host_tool(STATUS_TOOL, json!({})).await)
    }
}

#[cfg(test)]
#[path = "portal_present_tests.rs"]
mod tests;
