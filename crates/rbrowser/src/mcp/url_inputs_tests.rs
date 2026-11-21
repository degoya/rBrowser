//! Every URL an MCP tool takes, held to this module (security review w28): the tools come from
//! the generated `schema/mcp-tools.v1.json` (`RB_UPDATE_SCHEMA=1 ... mcp_tools_file`), so a new
//! tool with a `url`, `urls` or `...Url` input fails here until it is listed - in [`POLICED`]
//! with the file that calls `url_policy` for it, or in [`OWN_CHECK`] with where its URL is held
//! instead and why. A listed tool the schema does not have yet (one on another branch) is
//! skipped, so the table holds before and after its merge.

use std::path::Path;

use serde_json::Value;

/// Tools whose URL goes through `crate::mcp::url_policy`, and the source file (below
/// `crates/rbrowser/src/`) that calls it for them.
const POLICED: &[(&str, &str)] = &[
    // `view.navigate`, checked per target by the MCP backend.
    ("navigate", "mcp/backend.rs"),
    // RB-094-03: an agent compares the pages `navigate` may load.
    ("compare_engines", "engine_compare/plan.rs"),
    // RB-094-10: `checks.run_all`'s `url` and `breakpoints.analyze`'s `urls`, checked by the
    // MCP backend like `navigate`'s (`url_policy::check_command`).
    ("checks_run", "mcp/backend.rs"),
    ("breakpoints_detect", "mcp/backend.rs"),
];

/// Tools whose URL is held elsewhere, with where and why.
const OWN_CHECK: &[(&str, &str)] = &[(
    "sitemap_capture",
    "sitemap::plan: an http(s) site or sitemap, or a project-relative list file inside the project",
)];

fn repo() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
}

/// Whether a property name is a URL input: `url`, `urls`, `href`, or ending in `Url`/`Urls`.
fn is_url_name(name: &str) -> bool {
    matches!(name, "url" | "urls" | "href") || name.ends_with("Url") || name.ends_with("Urls")
}

fn has_url_input(schema: &Value) -> bool {
    let nested = |value: &Value| has_url_input(value);
    schema
        .get("properties")
        .and_then(Value::as_object)
        .is_some_and(|properties| {
            properties
                .iter()
                .any(|(name, value)| is_url_name(name) || nested(value))
        })
        || schema.get("items").is_some_and(nested)
}

fn tools() -> Vec<(String, bool)> {
    let text = std::fs::read_to_string(repo().join("schema/mcp-tools.v1.json"))
        .expect("the generated tool list");
    let value: Value = serde_json::from_str(&text).expect("JSON");
    value["tools"]
        .as_array()
        .expect("tools")
        .iter()
        .map(|tool| {
            let name = tool["name"].as_str().unwrap_or_default().to_owned();
            (name, has_url_input(&tool["inputSchema"]))
        })
        .collect()
}

#[test]
fn every_tool_that_takes_a_url_is_listed() {
    let unlisted: Vec<String> = tools()
        .into_iter()
        .filter(|(_, url)| *url)
        .map(|(name, _)| name)
        .filter(|name| {
            !POLICED.iter().any(|(tool, _)| tool == name)
                && !OWN_CHECK.iter().any(|(tool, _)| tool == name)
        })
        .collect();
    assert!(
        unlisted.is_empty(),
        "MCP tools take a URL without passing url_policy or naming their own check: {unlisted:?}"
    );
    assert!(
        tools().iter().any(|(name, url)| name == "navigate" && *url),
        "the scan finds navigate's url"
    );
}

#[test]
fn every_policed_tool_still_calls_the_policy() {
    let present = tools();
    for (tool, file) in POLICED {
        if !present.iter().any(|(name, _)| name == tool) {
            continue;
        }
        let source = std::fs::read_to_string(repo().join("crates/rbrowser/src").join(file))
            .unwrap_or_else(|error| panic!("{file} for {tool}: {error}"));
        assert!(
            source.contains("url_policy"),
            "{tool}: {file} no longer calls url_policy"
        );
    }
}

#[test]
fn navigate_refuses_what_the_policy_refuses() {
    for (command, args) in [
        (
            super::NAVIGATE,
            serde_json::json!({ "url": "file:///etc/passwd" }),
        ),
        (
            super::CHECKS_RUN,
            serde_json::json!({ "url": "file:///etc/passwd" }),
        ),
        (
            super::ANALYZE,
            serde_json::json!({ "urls": ["file:///etc/passwd"] }),
        ),
    ] {
        let refused = super::check_command(command, &args, Some(Path::new("/project")));
        assert!(refused.is_err(), "{command}");
    }
}
