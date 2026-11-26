//! iOS on macOS (RB-095-04): `ios_devices` lists the iOS simulators Xcode has and the iPhones
//! and iPads on USB, with the UDIDs `views_set_engine { engine: "ios", iosDevice }` takes; the
//! host answers it (`crates/rbrowser/src/engines/ios/commands.rs`). A read: it boots no
//! simulator and changes no setting.

use rmcp::handler::server::wrapper::Parameters;
use rmcp::{tool, tool_router};
use serde_json::json;

use super::NoArgs;
use crate::RbMcpServer;
use crate::answer::{ToolResult, respond};
use crate::backend::ToolFailure;

/// The host tool listing the simulators and devices.
const DEVICES_TOOL: &str = "ios_devices";

#[tool_router(router = ios_router, vis = "pub(crate)")]
impl RbMcpServer {
    #[tool(
        description = "The iOS simulators and devices of this Mac for iOS views (macOS only): supported (false elsewhere, with reason), xcode, simulators (udid, name, runtime, booted), devices (udid, name), mode (engines.ios.mode: auto, simulator, device), defaultSimulator, and default - the target a view without iosDevice gets - or reason, code and setting when there is none (engines.ios.no.way). Pass a name or udid to views_set_engine { engine: \"ios\", iosDevice }. Boots no simulator."
    )]
    pub(crate) async fn ios_devices(&self, _args: Parameters<NoArgs>) -> ToolResult {
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
