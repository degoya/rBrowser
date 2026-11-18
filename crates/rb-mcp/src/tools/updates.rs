//! The self-update (RB-090-14): `update_status`, `update_check` and `update_install`, all run by
//! the host (`crates/rbrowser/src/updates/`). The status and the check are reads of the update
//! (the check reaches the release host). The install installs nothing: rBrowser shows the offer
//! with its "Install and restart" button in the status bar, and only the person's click
//! downloads, verifies and switches; the command `update.install` itself is refused to MCP.

use rmcp::tool;
use rmcp::tool_router;
use serde_json::json;

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
        description = "rBrowser's self-update: enabled (updates.enabled), channel (stable or beta: updates.channel, whose default auto reads beta for a pre-release build and stable otherwise), installed (the running version), offered (the newest newer version the last check found, or null) with releasedAt, size in bytes and notes, installKind (portable, msi, scoop, winget, homebrew, deb, rpm or unknown) and updateWay (self_update; command - a package manager's, given in command, nothing is downloaded; download_only - the release page), state (idle, checking, up_to_date, available, downloading, ready, installing, failed), received bytes while downloading, problem (the last failure's code, update.*), autoInstall (off or onQuit), installsOnQuit and otherWindows (other projects' rBrowser windows; while any runs, nothing is installed). A read."
    )]
    pub(crate) async fn update_status(&self) -> ToolResult {
        respond(self.backend.host_tool(STATUS_TOOL, json!({})).await)
    }

    #[tool(
        description = "Check for an rBrowser update now: fetches the signed manifests of the channel from the GitHub releases, verifies them against the update key built into rBrowser, and answers the status as update_status does. A newer version is also shown to the person in rBrowser's status bar. Installs nothing. Off while updates.enabled is off (problem update.disabled)."
    )]
    pub(crate) async fn update_check(&self) -> ToolResult {
        respond(self.backend.host_tool(CHECK_TOOL, json!({})).await)
    }

    #[tool(
        description = "Ask the person to install the offered rBrowser update (checks first when nothing is offered yet). rBrowser shows the offer with an Install and restart button in its status bar and answers asked true, installed false: only the person's click downloads, verifies and installs it, and rBrowser restarts. No tool grants the confirmation, and the command update.install is refused to MCP. Nothing offered: asked false with the status. Off while updates.enabled is off (code update.disabled, settingsKey)."
    )]
    pub(crate) async fn update_install(&self) -> ToolResult {
        respond(self.backend.host_tool(INSTALL_TOOL, json!({})).await)
    }
}

#[cfg(test)]
#[path = "updates_tests.rs"]
mod tests;
