//! The plugins' MCP tools on the host's side (RB-060-12): what `rb_mcp` lists
//! (`Backend::plugin_tools`) and how a call reaches the plugin (`Backend::host_tool`, after the
//! core's own tools). Only the tools of plugins the window's runtime serves - a project plugin's
//! only under workspace trust; none before it started, so a listing never waits for the plugins
//! to compile.

use rb_mcp::{PluginToolInfo, ToolFailure};
use rb_plugin_host::PluginTool;
use serde_json::Value;

use super::for_host;
use crate::control::Core;

/// The plugin tools `core`'s window serves.
pub fn tools(core: &Core) -> Vec<PluginToolInfo> {
    served(core)
        .into_iter()
        .map(|tool| PluginToolInfo {
            description: rb_i18n::t(&tool.description),
            name: tool.name,
            input_schema: tool.input_schema,
        })
        .collect()
}

fn served(core: &Core) -> Vec<PluginTool> {
    let Some(runtime) = for_host(core.host()) else {
        return Vec::new();
    };
    crate::analyzer::plugins::bundled()
        .registry
        .tools()
        // A project plugin's tools only while the workspace is trusted (review M2, Charge 6).
        .filter(|tool| runtime.serves(&tool.plugin_id))
        .cloned()
        .collect()
}

/// A plugin tool that runs its caller's JavaScript (`evaluates_input`) answers only while the
/// person allows `evaluate` over MCP - the core `evaluate` tool's switch (Charge 6).
///
/// # Errors
///
/// `evaluate_disabled` naming the setting.
pub fn allowed(tool: &PluginTool, allow_evaluate: bool) -> Result<(), ToolFailure> {
    if !tool.evaluates_input || allow_evaluate {
        return Ok(());
    }
    Err(ToolFailure::new(
        rb_mcp::tools::page::EVALUATE_DISABLED,
        "this plugin tool runs page JavaScript and is switched off with evaluate; a person can \
         switch it on in rBrowser at Settings > Security & Privacy > Evaluate \
         (rbrowser://settings/mcp.allowEvaluate)",
    )
    .with_setting(rb_mcp::tools::page::ALLOW_EVALUATE))
}

/// Runs plugin tool `name` with `args` (`target` already `"all"` or a view id); `None` when no
/// served plugin has a tool of that name.
pub async fn call(core: &Core, name: &str, args: Value) -> Option<Result<Value, ToolFailure>> {
    let tool = served(core).into_iter().find(|tool| tool.name == name)?;
    let allow_evaluate = core.user_settings().mcp.allow_evaluate;
    if let Err(refused) = allowed(&tool, allow_evaluate) {
        return Some(Err(refused));
    }
    // RB-080-37: the debugger's plugin tools keep the core tools' gates over MCP.
    if let Err(refused) = crate::mcp::network_tools::plugin_gate(name, allow_evaluate) {
        return Some(Err(refused));
    }
    let runtime = for_host(core.host())?;
    let result = runtime
        .call_tool(&tool.plugin_id, &tool.tool, &args)
        .await
        .map_err(|failure| {
            let code = failure
                .code()
                .unwrap_or("plugins.error.host_failed")
                .to_owned();
            let message = super::commands::failure_message(&tool.plugin_id, &failure);
            ToolFailure::new(code, message)
        });
    Some(result)
}

#[cfg(test)]
#[path = "mcp_tests.rs"]
mod tests;
