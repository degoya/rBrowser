//! The MCP tools of the self-update (RB-090-14), run by the host (`mcp/host_tools.rs`):
//! `update_status` (channel, installed and offered version, size, notes, install kind and how it
//! updates, the state and the last problem), `update_check` (one check now, then the status) and
//! `update_install`, which installs nothing: it shows the offer with its "Install and restart"
//! button in the status bar, and only the person's click runs `update.install`. A build without
//! the feature `self-update` answers `update.not_built`.

use std::sync::Arc;

use rb_mcp::ToolFailure;
use serde_json::Value;

use crate::control::Core;

/// `update_status`.
pub const STATUS_TOOL: &str = "update_status";
/// `update_check`.
pub const CHECK_TOOL: &str = "update_check";
/// `update_install`.
pub const INSTALL_TOOL: &str = "update_install";

/// Whether `name` is one of the tools here.
#[must_use]
pub fn is_tool(name: &str) -> bool {
    [STATUS_TOOL, CHECK_TOOL, INSTALL_TOOL].contains(&name)
}

/// Runs the tool `name` off the async workers (a check reaches the network).
///
/// # Errors
///
/// `update.not_built`, `update.disabled` for an install while updates are off, `unfinished` when
/// the blocking thread is gone.
pub async fn call(core: &Arc<Core>, name: &str) -> Result<Value, ToolFailure> {
    #[cfg(feature = "self-update")]
    {
        let core = core.clone();
        let name = name.to_owned();
        let runtime = core.runtime().clone();
        runtime
            .spawn_blocking(move || super::flow::mcp(&core, &name))
            .await
            .map_err(|_| ToolFailure::new("update.unfinished", "the update tool did not finish"))?
    }
    #[cfg(not(feature = "self-update"))]
    {
        let _ = core;
        Err(ToolFailure::new(
            "update.not_built",
            format!("{name}: this build carries no self-update"),
        ))
    }
}
