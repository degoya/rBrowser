use rb_plugin_api::feature::Feature;
use serde_json::{Value, json};

use super::*;
use crate::testing::FakeHost;

#[test]
fn toggle_posts_to_every_view() {
    let mut host = FakeHost::default();
    let answer: Value = serde_json::from_str(
        &Peek
            .run_command(&mut host, "toggle", "{}")
            .expect("answers"),
    )
    .expect("JSON");
    assert_eq!(
        answer,
        json!({ "$post": [{ "target": "all", "message": { "type": "toggle" } }] })
    );
}

#[test]
fn the_tools_and_the_page_are_served_and_nothing_else() {
    let mut host = FakeHost::default();
    let config = Peek
        .call_tool(
            &mut host,
            PAGE_TOOL,
            &json!({ "view": 1, "message": { "type": "config" } }).to_string(),
        )
        .expect("answers");
    assert!(config.contains("\"modifier\":\"Alt\""));
    host.evaluate.insert(1, "null".to_owned());
    let inspected = Peek
        .call_tool(&mut host, "inspect", r#"{"target":1,"selector":"p"}"#)
        .expect("answers");
    assert!(inspected.contains("\"found\":false"));
    let failure = Peek
        .call_tool(&mut host, "launch", "{}")
        .expect_err("unknown");
    assert_eq!(failure.code.as_deref(), Some(codes::UNKNOWN_CONTRIBUTION));
    let failure = Peek
        .run_command(&mut host, "launch", "{}")
        .expect_err("unknown");
    assert_eq!(failure.code.as_deref(), Some(codes::UNKNOWN_CONTRIBUTION));
}

#[test]
fn the_manifest_declares_what_the_component_serves() {
    let manifest = include_str!("../manifest.toml");
    assert!(manifest.contains("id = \"peek\""));
    assert!(manifest.contains("mcp_prefix = \"peek\""));
    assert!(manifest.contains("name = \"inspect\""));
    assert!(manifest.contains("id = \"toggle\""));
    assert!(manifest.contains("shortcut = \"Ctrl+Shift+Space D P\""));
    assert!(manifest.contains("domains = [\"DOM\", \"CSS\", \"Accessibility\"]"));
}
