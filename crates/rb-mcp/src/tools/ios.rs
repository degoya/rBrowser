//! iOS on macOS (RB-095-04): `ios_devices` lists the iOS simulators Xcode has and the iPhones
//! and iPads on USB, with the UDIDs `views_set_engine { engine: "ios", iosDevice }` takes; the
//! host answers it (`crates/rbrowser/src/engines/ios/commands.rs`). A read: it boots no
//! simulator and changes no setting.

use rmcp::{tool, tool_router};
use serde_json::json;

use crate::RbMcpServer;
use crate::answer::{ToolResult, respond};
use crate::backend::ToolFailure;

/// The host tool listing the simulators and devices.
const DEVICES_TOOL: &str = "ios_devices";

#[tool_router(router = ios_router, vis = "pub(crate)")]
impl RbMcpServer {
    #[tool(
        description = "The iOS simulators and devices of this Mac for iOS views (Mobile Safari over safaridriver, macOS only): supported (false outside macOS, with reason), xcode (Xcode installed - the iOS Simulator needs it), simulators (udid, name such as \"iPhone 16\", runtime such as \"iOS 18.2\", booted), devices (iPhones and iPads on USB: udid, name), mode (engines.ios.mode: auto - the simulator when Xcode is installed, else a connected device -, simulator or device), defaultSimulator and default, the target a view without iosDevice gets (udid, label, simulator) - or reason, code and setting when there is none (engines.ios.no.way: neither Xcode nor a device; install Xcode for the simulator or connect an iPhone with Settings > Safari > Advanced > Remote Automation on). Put a name or a udid into views_set_engine { engine: \"ios\", iosDevice }. A read: it boots no simulator."
    )]
    pub(crate) async fn ios_devices(&self) -> ToolResult {
        let result = async {
            let answer = self.backend.host_tool(DEVICES_TOOL, json!({})).await?;
            Ok::<_, ToolFailure>(self.backend.mask(&answer))
        }
        .await;
        respond(result)
    }
}

#[cfg(test)]
#[path = "ios_tests.rs"]
mod tests;
