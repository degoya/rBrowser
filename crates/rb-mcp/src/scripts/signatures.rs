//! The `rb.*` signatures in `script_run`'s description: every tool the session serves - the
//! core's, which `schema/mcp-tools.v1.json` is written from, and the loaded plugins' - as
//! `rb.<name>({ argument, optional? })`, so a model writes the script without asking first.
//! Built when the session's server is made ([`describe`]), so a plugin tool is in it as soon as
//! it is listed.

use std::borrow::Cow;

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::model::Tool;
use serde_json::Value;

use super::TOOL;
use crate::RbMcpServer;

/// One line per tool, by name; `script_run` itself is left out.
#[must_use]
pub fn signatures(tools: &[Tool]) -> String {
    let mut tools: Vec<&Tool> = tools.iter().filter(|tool| tool.name != TOOL).collect();
    tools.sort_by(|a, b| a.name.cmp(&b.name));
    tools
        .iter()
        .map(|tool| signature(tool))
        .collect::<Vec<_>>()
        .join("\n")
}

/// `rb.<name>({ a, b? })`.
fn signature(tool: &Tool) -> String {
    let required: Vec<&str> = tool
        .input_schema
        .get("required")
        .and_then(Value::as_array)
        .map(|list| list.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    let arguments: Vec<String> = tool
        .input_schema
        .get("properties")
        .and_then(Value::as_object)
        .map(|properties| {
            properties
                .keys()
                .map(|key| {
                    if required.contains(&key.as_str()) {
                        key.clone()
                    } else {
                        format!("{key}?")
                    }
                })
                .collect()
        })
        .unwrap_or_default();
    if arguments.is_empty() {
        format!("rb.{}({{}})", tool.name)
    } else {
        format!("rb.{}({{ {} }})", tool.name, arguments.join(", "))
    }
}

/// `base` with the signatures of `tools` after it.
#[must_use]
pub fn description(base: &str, tools: &[Tool]) -> String {
    format!(
        "{base}\n\nInside the script, rb has one async method per tool (the argument is the \
         tool's arguments object; the promise gives the tool's JSON answer):\n{}",
        signatures(tools)
    )
}

/// Writes the signatures of every tool `router` serves into its `script_run` description.
pub fn describe(router: &mut ToolRouter<RbMcpServer>) {
    let tools = router.list_all();
    if let Some(route) = router.map.get_mut(TOOL) {
        let base = route.attr.description.as_deref().unwrap_or_default();
        let full = description(base, &tools);
        route.attr.description = Some(Cow::Owned(full));
    }
}

#[cfg(test)]
#[path = "signatures_tests.rs"]
mod tests;
