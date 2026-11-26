//! The portal (RB-080-05): `portal_stop`, `portal_devices` and `portal_disconnect`, run by the
//! host (`crates/rbrowser/src/portal/mcp.rs`). No tool starts the portal: a listener in the LAN
//! is a person's decision (`portal.enabled`, a protected key MCP cannot write, and then
//! `portal.toggle` in rBrowser; Charge 10 review L1). Pairing stays a person's act too: the
//! one-time pairing code is shown in rBrowser's Portal panel and never reaches an MCP answer, so
//! MCP can neither pair a device nor read a device's session. The tools act on the portal, not
//! on views: none takes `target`.

use rmcp::{handler::server::wrapper::Parameters, schemars, tool, tool_router};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::RbMcpServer;
use crate::answer::{ToolResult, respond};

/// `portal_stop`.
pub const STOP_TOOL: &str = "portal_stop";
/// `portal_devices`.
pub const DEVICES_TOOL: &str = "portal_devices";
/// `portal_disconnect`.
pub const DISCONNECT_TOOL: &str = "portal_disconnect";

/// `portal_stop` and `portal_devices`: no arguments.
#[derive(Debug, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PortalParams {}

/// `portal_disconnect`.
#[derive(Debug, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PortalDisconnectParams {
    /// The device's id from `portal_devices`; every device when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device: Option<u32>,
}

#[tool_router(router = portal_router, vis = "pub(crate)")]
impl RbMcpServer {
    #[tool(
        description = "Stop the portal: it stops listening and every device's connection ends. Paired devices must pair again after the next start."
    )]
    pub(crate) async fn portal_stop(&self, Parameters(_): Parameters<PortalParams>) -> ToolResult {
        respond(self.backend.host_tool(STOP_TOOL, json!({})).await)
    }

    #[tool(
        description = "The portal's state: whether it may run (portal.enabled) and runs, its address and the paired devices (id, name, ip, pairedAtMs, lastSeenMs, connected, and the events replayed, failed and skipped). MCP cannot start the portal or pair a device: a person does, with a one-time QR code that never reaches MCP."
    )]
    pub(crate) async fn portal_devices(
        &self,
        Parameters(_): Parameters<PortalParams>,
    ) -> ToolResult {
        respond(self.backend.host_tool(DEVICES_TOOL, json!({})).await)
    }

    #[tool(
        description = "Disconnect a paired device (device: its id from portal_devices) or, without device, every device: the device's session is revoked at once, its connection closes and its next request gets 403. Answers how many were disconnected."
    )]
    pub(crate) async fn portal_disconnect(
        &self,
        Parameters(params): Parameters<PortalDisconnectParams>,
    ) -> ToolResult {
        let args: Value = params
            .device
            .map_or_else(|| json!({}), |device| json!({ "device": device }));
        respond(self.backend.host_tool(DISCONNECT_TOOL, args).await)
    }
}

#[cfg(test)]
#[path = "portal_tests.rs"]
mod tests;
