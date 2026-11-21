//! Android devices over ADB (RB-095-02): `android_devices` lists the USB devices adb sees and
//! `view_add_android { serial? }` adds one as a view at the device's own size; the host runs
//! both (`crates/rbrowser/src/engines/android/commands.rs`). Neither switches Android views on
//! nor downloads adb: `engines.android.enabled`, `engines.android.adbPath` and the Platform
//! Tools download are the person's.

use rmcp::{handler::server::wrapper::Parameters, schemars, tool, tool_router};
use serde::{Deserialize, Serialize};
use serde_json::json;

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
        description = "The Android devices on USB that adb sees, for Android views (Chrome on the device over ADB): enabled (engines.android.enabled, off by default - only the person switches it on), adb (the program in use and where it was found: setting, path, androidHome, sdkRoot, usualSdk or managed) and per device its serial, state (device, unauthorized - allow USB debugging on the device -, offline, noPermissions), model, ready and the breakpoint id a view of it gets (android-<serial>). reason says why nothing is listed (off, no adb). A read: it never downloads adb."
    )]
    pub(crate) async fn android_devices(&self) -> ToolResult {
        let result = async {
            let answer = self.backend.host_tool(DEVICES_TOOL, json!({})).await?;
            Ok::<_, ToolFailure>(self.backend.mask(&answer))
        }
        .await;
        respond(result)
    }

    #[tool(
        description = "Add an Android device on USB as a view: Chrome on the device over ADB with its DevTools port forwarded to 127.0.0.1 only, at the device's own size (no width emulation; the label says device size), as the breakpoint android-<serial> in config.local.json. serial from android_devices; without it the one ready device. The view follows the navigations of the grid; screenshot works on it. Answers the breakpoint, width, height, dpr, added, changed and serial. Refused with android.disabled (engines.android.enabled is off - the person's switch), android.no_adb (no adb found; the person sets engines.android.adbPath or downloads the Platform Tools in rBrowser - never over MCP), android.no_device or android.device_not_ready (with the reason, e.g. USB debugging not allowed)."
    )]
    pub(crate) async fn view_add_android(
        &self,
        Parameters(params): Parameters<ViewAddAndroidParams>,
    ) -> ToolResult {
        let result = async {
            let args = serde_json::to_value(&params)
                .map_err(|error| ToolFailure::new("internal", error.to_string()))?;
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
