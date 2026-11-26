//! Android devices over ADB (RB-095-02): `android_devices` lists the USB devices adb sees and
//! `view_add_android { serial? }` adds one as a view at the device's own size; the host runs
//! both (`crates/rbrowser/src/engines/android/commands.rs`). Neither switches Android views on
//! nor downloads adb: `engines.android.enabled`, `engines.android.adbPath` and the Platform
//! Tools download are the person's.

use rmcp::{handler::server::wrapper::Parameters, schemars, tool, tool_router};
use serde::{Deserialize, Serialize};
use serde_json::json;

use super::NoArgs;
use crate::RbMcpServer;
use crate::answer::{ToolResult, respond};
use crate::backend::ToolFailure;

/// The host tool listing the devices.
const DEVICES_TOOL: &str = "android_devices";
/// The host tool adding one.
const ADD_TOOL: &str = "view_add_android";

/// `view_add_android`.
#[derive(Debug, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ViewAddAndroidParams {
    /// The device's serial from android_devices; the one ready device when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub serial: Option<String>,
}

#[tool_router(router = android_router, vis = "pub(crate)")]
impl RbMcpServer {
    #[tool(
        description = "The Android devices on USB that adb sees, for Android views: enabled (engines.android.enabled, the person's switch), adb (the program in use and where it was found) and per device serial, state (device, unauthorized, offline, noPermissions), model, ready and the breakpoint id its view gets (android-<serial>). reason says why nothing is listed. Never downloads adb."
    )]
    pub(crate) async fn android_devices(&self, _args: Parameters<NoArgs>) -> ToolResult {
        let result = async {
            let answer = self.backend.host_tool(DEVICES_TOOL, json!({})).await?;
            Ok::<_, ToolFailure>(self.backend.mask(&answer))
        }
        .await;
        respond(result)
    }

    #[tool(
        description = "Add an Android device on USB as a view: Chrome on the device over ADB (its DevTools port forwarded to 127.0.0.1 only) at the device's own size, as the breakpoint android-<serial> in config.local.json. serial from android_devices; the one ready device when absent. The view follows the grid's navigations. Answers breakpoint, width, height, dpr, added, changed and serial. Refused with android.disabled (engines.android.enabled), android.no_adb (the adb path or download is the person's, never MCP's), android.no_device or android.device_not_ready (with the reason)."
    )]
    pub(crate) async fn view_add_android(
        &self,
        Parameters(params): Parameters<ViewAddAndroidParams>,
    ) -> ToolResult {
        let result = async {
            let args = serde_json::to_value(&params)
                .map_err(|error| ToolFailure::new("mcp.internal", error.to_string()))?;
            let answer = self.backend.host_tool(ADD_TOOL, args).await?;
            Ok::<_, ToolFailure>(self.backend.mask(&answer))
        }
        .await;
        respond(result)
    }
}

#[cfg(test)]
#[path = "android_tests.rs"]
mod tests;
