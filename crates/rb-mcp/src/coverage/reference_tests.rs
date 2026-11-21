use std::fs;
use std::path::Path;

use serde_json::Value;

use super::{REGENERATE, TOOLS_FILE, document};
use crate::RbMcpServer;

fn parsed() -> Value {
    serde_json::from_str(&document()).expect("the document is JSON")
}

#[test]
fn every_tool_is_in_the_document_by_name_with_its_schema() {
    let document = parsed();
    let tools = document["tools"].as_array().expect("tools");
    let names: Vec<&str> = tools
        .iter()
        .map(|tool| tool["name"].as_str().expect("name"))
        .collect();
    assert_eq!(names, RbMcpServer::tool_names());
    for tool in tools {
        assert_eq!(tool["inputSchema"]["type"], "object", "{tool}");
        assert!(
            tool["description"]
                .as_str()
                .is_some_and(|text| !text.is_empty())
        );
        assert!(
            ["all", "one", "none"].contains(&tool["target"].as_str().unwrap_or("")),
            "{tool}"
        );
    }
    assert_eq!(document["version"], 1);
    // One entry per pending tool (none since RB-040-05 registered `cookies_copy`).
    assert_eq!(
        document["pending"].as_array().map(Vec::len),
        Some(crate::coverage::PENDING.len())
    );
}

#[test]
fn the_document_says_targets_guards_and_reads() {
    let document = parsed();
    let tool = |name: &str| {
        document["tools"]
            .as_array()
            .and_then(|tools| tools.iter().find(|tool| tool["name"] == name))
            .cloned()
            .expect(name)
    };
    assert_eq!(tool("navigate")["target"], "all");
    assert_eq!(tool("devtools_open")["target"], "one");
    assert_eq!(tool("settings_get")["target"], "none");
    assert_eq!(tool("settings_get")["actsOn"], "the settings");
    assert_eq!(tool("evaluate")["guard"]["setting"], "mcp.allowEvaluate");
    assert!(tool("views_list")["withoutCommand"].is_string());
    assert!(tool("navigate").get("withoutCommand").is_none());
}

/// `schema/mcp-tools.v1.json` is generated from the tool definitions; this test fails when it
/// is stale. Regenerate with `RB_UPDATE_SCHEMA=1 cargo nextest run -p rb-mcp mcp_tools_file`.
#[test]
fn mcp_tools_file_is_current() {
    let text = document();
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(TOOLS_FILE);
    if std::env::var_os("RB_UPDATE_SCHEMA").is_some_and(|value| value == "1") {
        fs::write(&path, &text).expect("write the tools file");
    }
    assert!(
        fs::read_to_string(&path).ok().as_deref() == Some(text.as_str()),
        "{TOOLS_FILE} is stale; regenerate with {REGENERATE}"
    );
}

#[test]
fn the_document_lists_the_prompts_and_resources() {
    let document = parsed();
    let names: Vec<&str> = document["prompts"]
        .as_array()
        .expect("prompts")
        .iter()
        .filter_map(|prompt| prompt["name"].as_str())
        .collect();
    assert_eq!(names, crate::prompts::NAMES);
    assert_eq!(
        document["prompts"][0]["arguments"][1]["name"],
        "breakpoints"
    );
    let uris: Vec<&str> = document["resources"]
        .as_array()
        .expect("resources")
        .iter()
        .filter_map(|resource| resource["uri"].as_str())
        .collect();
    assert_eq!(
        uris,
        [
            crate::resources::VIEWS,
            crate::resources::FINDINGS,
            crate::resources::SCREENSHOTS,
            crate::resources::CONFIG
        ]
    );
    assert_eq!(
        document["resourceTemplates"][0]["uriTemplate"],
        crate::resources::SCREENSHOT_TEMPLATE
    );
}
