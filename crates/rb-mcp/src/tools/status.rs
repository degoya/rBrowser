//! The status bar (RB-080-15): `status_messages`, a read the host answers
//! (`Backend::host_tool`) from the window's message log. It acts on no view. The commands
//! `status.toggle_bar` and `status.history` have no tool (a display switch and a popup); this
//! is their data. The answer is masked: a message may carry the CDP endpoint with its token.

use rmcp::{handler::server::wrapper::Parameters, schemars, tool, tool_router};
use serde::{Deserialize, Serialize};

use crate::RbMcpServer;
use crate::answer::{ToolResult, respond};
use crate::backend::ToolFailure;

/// The host tool behind `status_messages`.
pub const STATUS_MESSAGES_TOOL: &str = "status_messages";

/// A message's level.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum StatusLevel {
    /// Information.
    Info,
    /// Something to look at.
    Warning,
    /// Something failed.
    Error,
    /// Something finished well.
    Success,
}

/// `status_messages`.
#[derive(Debug, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StatusMessagesParams {
    /// At most this many messages, 1 to 1000; 50 when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    /// Only messages of this level.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<StatusLevel>,
}

#[tool_router(router = status_router, vis = "pub(crate)")]
impl RbMcpServer {
    #[tool(
        description = "Read the status bar's last messages - what rBrowser told the person (errors, warnings, info, success), newest first: id, at (ms since the epoch), level, messageKey, the English text and its settingsKey, if any; kept (ui.statusBar.historySize). limit 1 to 1000 (default 50)."
    )]
    pub(crate) async fn status_messages(
        &self,
        Parameters(params): Parameters<StatusMessagesParams>,
    ) -> ToolResult {
        let result = async {
            let args = serde_json::to_value(&params)
                .map_err(|error| ToolFailure::new("mcp.internal", error.to_string()))?;
            let answer = self.backend.host_tool(STATUS_MESSAGES_TOOL, args).await?;
            Ok(self.backend.mask(&answer))
        }
        .await;
        respond(result)
    }
}

#[cfg(test)]
#[path = "status_tests.rs"]
mod tests;
