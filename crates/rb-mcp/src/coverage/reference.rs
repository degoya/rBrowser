//! `schema/mcp-tools.v1.json`: every tool as an agent sees it (name, description, the schema of
//! its arguments) plus what the coverage tables say about it (area, `target`, guard, why no
//! command runs it), the pending tools with their jobs, and the prompts and resources with their
//! arguments and URIs (RB-095-01). A test writes it
//! (`RB_UPDATE_SCHEMA=1`) and fails while it is stale; `scripts/mcp-reference.sh` renders the
//! wiki's `reference/mcp-tools.md` from it and `schema/commands.v1.json`, compiling nothing.

use serde_json::{Map, Value, json};

use super::PENDING;
use super::tables;
use super::tools::{areas, target_support};

/// The file, from the repository root.
pub const TOOLS_FILE: &str = "schema/mcp-tools.v1.json";
/// How the file is regenerated.
pub const REGENERATE: &str = "RB_UPDATE_SCHEMA=1 cargo nextest run -p rb-mcp mcp_tools_file";

/// The text of [`TOOLS_FILE`]: pretty JSON with a trailing newline, the tools by name.
#[must_use]
pub fn document() -> String {
    let mut tools: Vec<Value> = areas()
        .into_iter()
        .flat_map(|(area, tools)| tools.into_iter().map(move |tool| (area, tool)))
        .map(|(area, tool)| {
            let name = tool.name.as_ref();
            let mut entry = Map::new();
            entry.insert("name".to_owned(), json!(name));
            entry.insert("area".to_owned(), json!(area));
            entry.insert("description".to_owned(), json!(tool.description));
            let target = target_support(&tool).map_or("invalid", |support| support.as_str());
            entry.insert("target".to_owned(), json!(target));
            if let Some(acts_on) = tables::untargeted(name) {
                entry.insert("actsOn".to_owned(), json!(acts_on));
            }
            if let Some(guard) = tables::guard(name) {
                let mut value = json!({ "note": guard.note });
                if let Some(setting) = guard.setting {
                    value["setting"] = json!(setting);
                }
                entry.insert("guard".to_owned(), value);
            }
            if let Some(reason) = tables::without_command(name) {
                entry.insert("withoutCommand".to_owned(), json!(reason));
            }
            entry.insert(
                "inputSchema".to_owned(),
                Value::Object((*tool.input_schema).clone()),
            );
            Value::Object(entry)
        })
        .collect();
    tools.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
    let pending: Vec<Value> = PENDING
        .iter()
        .map(|(tool, job)| json!({ "tool": tool, "job": job }))
        .collect();
    let document = json!({
        "$comment": format!(
            "Generated from rb-mcp's tool definitions by `{REGENERATE}`; do not edit by hand."
        ),
        "version": 1,
        "tools": tools,
        "pending": pending,
        "prompts": crate::prompts::list(),
        "resources": crate::resources::list(),
        "resourceTemplates": crate::resources::templates(),
    });
    let mut text = serde_json::to_string_pretty(&document).expect("JSON values serialise");
    text.push('\n');
    text
}

#[cfg(test)]
#[path = "reference_tests.rs"]
mod tests;
