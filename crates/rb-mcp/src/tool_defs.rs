//! The core tools' definitions for clients outside MCP (RB-098-01): `rbrowser mcp tools
//! --format anthropic` prints them as the Messages API's custom tools, so a program using
//! Anthropic's programmatic tool calling declares rBrowser's tools itself (with
//! `allowed_callers`) and forwards each call over MCP. The same definitions as
//! `schema/mcp-tools.v1.json` and `tools/list` - `script_run` with its `rb.*` signatures - without
//! the plugins' tools, which only the running rBrowser knows.

use serde_json::{Value, json};

use crate::RbMcpServer;

/// The shape of the definitions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolFormat {
    /// As MCP's `tools/list` has them: `name`, `description`, `inputSchema`.
    Mcp,
    /// As the Messages API's custom tools: `name`, `description`, `input_schema`.
    Anthropic,
}

/// Every core tool in `format`, by name; `allowed_caller` adds `allowed_callers: [it]` to each
/// Anthropic definition (`code_execution_20260120` for programmatic tool calling).
#[must_use]
pub fn definitions(format: ToolFormat, allowed_caller: Option<&str>) -> Value {
    let mut router = RbMcpServer::router();
    crate::scripts::signatures::describe(&mut router);
    let tools = router
        .list_all()
        .into_iter()
        .map(|tool| {
            let schema = Value::Object((*tool.input_schema).clone());
            let description = tool.description.as_deref().unwrap_or_default().to_owned();
            match format {
                ToolFormat::Mcp => json!({
                    "name": tool.name, "description": description, "inputSchema": schema,
                }),
                ToolFormat::Anthropic => {
                    let mut entry = json!({
                        "name": tool.name, "description": description, "input_schema": schema,
                    });
                    if let Some(caller) = allowed_caller {
                        entry["allowed_callers"] = json!([caller]);
                    }
                    entry
                }
            }
        })
        .collect();
    Value::Array(tools)
}

#[cfg(test)]
#[path = "tool_defs_tests.rs"]
mod tests;
