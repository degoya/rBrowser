use serde_json::json;

use super::*;
use crate::fake_host::FakeHost;

#[test]
fn the_rules_request_answers_the_document() {
    let mut host = FakeHost::default();
    host.set("block", r#"["*/tracking.js"]"#);
    let answer = NetworkPanel
        .call_tool(
            &mut host,
            RULES_TOOL,
            r#"{"$network":{"session":"s","trusted":false}}"#,
        )
        .expect("answers");
    let answer: Value = serde_json::from_str(&answer).expect("JSON");
    assert_eq!(answer["rules"][0]["id"], "settings:block-0");
    assert_eq!(answer["cacheDisabled"], false);
}

#[test]
fn every_manifest_tool_and_command_is_served() {
    let manifest = include_str!("../manifest.toml");
    let mut host = FakeHost::default();
    for line in manifest.lines() {
        if let Some(name) = line
            .strip_prefix("name = \"")
            .and_then(|rest| rest.strip_suffix('"'))
            && name != "Network"
        {
            let result = NetworkPanel.call_tool(&mut host, name, "{}");
            assert!(
                result.as_ref().err().and_then(|f| f.code.as_deref())
                    != Some(codes::UNKNOWN_CONTRIBUTION),
                "tool {name}"
            );
        }
        if let Some(id) = line
            .strip_prefix("id = \"")
            .and_then(|rest| rest.strip_suffix('"'))
            && id != PLUGIN_ID
            && id != PANEL_ID
        {
            let result = NetworkPanel.run_command(&mut host, id, "{}");
            assert!(
                result.as_ref().err().and_then(|f| f.code.as_deref())
                    != Some(codes::UNKNOWN_CONTRIBUTION),
                "command {id}"
            );
        }
    }
    let unknown = NetworkPanel
        .call_tool(&mut host, "nope", "{}")
        .expect_err("unknown");
    assert_eq!(unknown.code.as_deref(), Some(codes::UNKNOWN_CONTRIBUTION));
}

#[test]
fn the_panel_command_asks_for_the_panel() {
    let mut host = FakeHost::default();
    let answer = NetworkPanel
        .run_command(&mut host, "panel", "")
        .expect("answers");
    assert_eq!(
        serde_json::from_str::<Value>(&answer).expect("JSON"),
        json!({ "$open": { "panel": "network" } })
    );
}

#[test]
fn arguments_that_are_no_object_are_refused() {
    let mut host = FakeHost::default();
    let refused = NetworkPanel
        .call_tool(&mut host, "list", "[1]")
        .expect_err("no object");
    assert_eq!(
        refused.code.as_deref(),
        Some("plugin.network-panel.errors.bad_args")
    );
}
