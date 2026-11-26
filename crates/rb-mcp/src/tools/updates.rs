//! The self-update (RB-090-14): `update_status`, `update_check` and `update_install`, all run by
//! the host (`crates/rbrowser/src/updates/`). The status and the check are reads of the update
//! (the check reaches the release host). The install installs nothing: rBrowser shows the offer
//! with its "Install and restart" button in the status bar, and only the person's click
//! downloads, verifies and switches; the command `update.install` itself is refused to MCP.

use rmcp::handler::server::wrapper::Parameters;
use rmcp::tool;
use rmcp::tool_router;
use serde_json::json;

use super::NoArgs;
use crate::RbMcpServer;
use crate::answer::{ToolResult, respond};

/// The host tool behind the status.
const STATUS_TOOL: &str = "update_status";
/// The host tool behind the check.
const CHECK_TOOL: &str = "update_check";
/// The host tool behind the install request.
const INSTALL_TOOL: &str = "update_install";

#[tool_router(router = updates_router, vis = "pub(crate)")]
impl RbMcpServer {
    #[tool(
        description = "rBrowser's self-update, a read: enabled (updates.enabled), channel (stable or beta), installed, offered (the newest newer version found, or null, with releasedAt, size, notes), installKind and updateWay (self_update, command - a package manager's, in command -, download_only), state (idle, checking, up_to_date, available, downloading, ready, installing, failed), received, problem (the last failure's code), autoInstall, installsOnQuit and otherWindows (while other windows run, nothing installs)."
    )]
    pub(crate) async fn update_status(&self, _args: Parameters<NoArgs>) -> ToolResult {
        respond(self.backend.host_tool(STATUS_TOOL, json!({})).await)
    }

    #[tool(
        description = "Check for an rBrowser update now: fetches the channel's signed manifests from the GitHub releases, verifies them against the built-in update key and answers as update_status does; a newer version is also shown in rBrowser's status bar. Installs nothing. Off while updates.enabled is off (code update.disabled)."
    )]
    pub(crate) async fn update_check(&self, _args: Parameters<NoArgs>) -> ToolResult {
        respond(self.backend.host_tool(CHECK_TOOL, json!({})).await)
    }

    #[tool(
        description = "Ask the person to install the offered rBrowser update (checks first when nothing is offered). rBrowser shows the offer with an Install and restart button; only the person's click installs it. Answers asked true, installed false; nothing offered: asked false with the status. Off while updates.enabled is off (code update.disabled)."
    )]
    pub(crate) async fn update_install(&self, _args: Parameters<NoArgs>) -> ToolResult {
        respond(self.backend.host_tool(INSTALL_TOOL, json!({})).await)
    }
}

#[cfg(test)]
#[path = "updates_tests.rs"]
mod tests;
