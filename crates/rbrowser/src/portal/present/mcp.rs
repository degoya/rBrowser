//! The MCP tools of the presentation (RB-096-05), run by the host for `rb_mcp`'s
//! `portal_present_*` tools through [`crate::portal::mcp`]: `portal_present_status` (phase,
//! viewers, the link's time left - never the link, its token or the address) and
//! `portal_present_stop`. No tool starts a presentation or issues a link: sharing the site on the
//! internet is a person's act.

use std::sync::Arc;

use rb_mcp::ToolFailure;
use serde_json::Value;

use super::{app, panel};
use crate::control::Core;

/// `portal_present_stop`.
pub const STOP_TOOL: &str = "portal_present_stop";
/// `portal_present_status`.
pub const STATUS_TOOL: &str = "portal_present_status";
/// Every tool, for the portal's dispatch.
pub const TOOLS: [&str; 2] = [STOP_TOOL, STATUS_TOOL];

/// Runs the presentation tool `name`; the status after it.
///
/// # Errors
///
/// `unknown_tool` for any other name.
pub fn host_tool(core: &Arc<Core>, name: &str) -> Result<Value, ToolFailure> {
    match name {
        STOP_TOOL => {
            app::stop(core);
        }
        STATUS_TOOL => {}
        other => {
            return Err(ToolFailure::new(
                "unknown_tool",
                format!("the presentation runs no tool `{other}`"),
            ));
        }
    }
    Ok(panel::status(core))
}
