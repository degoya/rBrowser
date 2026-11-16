//! The MCP tools of the portal (RB-080-05), run by the host for `rb_mcp`'s `portal_*` tools.
//! Their answers are the panel's state without the pairing: no pairing token and no device
//! session ever reaches MCP, so MCP can neither pair a device nor act as one. MCP cannot start
//! the portal either - a LAN listener is a person's decision (Charge 10 review L1); it can read
//! the state, disconnect devices and stop it.

use std::sync::Arc;

use rb_mcp::ToolFailure;
use serde_json::Value;

use super::app;
use crate::control::Core;

/// `portal_stop`.
pub const STOP_TOOL: &str = "portal_stop";
/// `portal_devices`.
pub const DEVICES_TOOL: &str = "portal_devices";
/// `portal_disconnect`.
pub const DISCONNECT_TOOL: &str = "portal_disconnect";
/// Every tool, for the host's dispatch.
pub const TOOLS: [&str; 3] = [STOP_TOOL, DEVICES_TOOL, DISCONNECT_TOOL];

/// Runs the portal tool `name` with `args`.
///
/// # Errors
///
/// `invalid_arguments` for a device that is no id, `unknown_tool` for any other name.
pub fn host_tool(core: &Arc<Core>, name: &str, args: &Value) -> Result<Value, ToolFailure> {
    match name {
        STOP_TOOL => {
            app::stop(core);
        }
        DEVICES_TOOL => {}
        DISCONNECT_TOOL => {
            let device = match args.get("device") {
                None | Some(Value::Null) => None,
                Some(value) => Some(
                    value
                        .as_u64()
                        .and_then(|id| u32::try_from(id).ok())
                        .ok_or_else(|| {
                            ToolFailure::new("invalid_arguments", "device is a device id")
                        })?,
                ),
            };
            let disconnected = super::disconnect(device);
            return Ok(serde_json::json!({ "disconnected": disconnected }));
        }
        other => {
            return Err(ToolFailure::new(
                "unknown_tool",
                format!("the portal runs no tool `{other}`"),
            ));
        }
    }
    serde_json::to_value(app::panel_state(core, false))
        .map_err(|error| ToolFailure::new("internal", error.to_string()))
}
