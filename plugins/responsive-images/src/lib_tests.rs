use rb_plugin_api::check::Check;
use rb_plugin_api::feature::Feature;
use rb_plugin_api::{CheckContext, codes};
use serde_json::{Value, json};

use super::*;
use crate::fake::FakeHost;

const DESKTOP: &str = include_str!("../tests/fixtures/desktop.json");

fn host() -> FakeHost {
    let mut host = FakeHost::default();
    host.view(1, "Desktop", "https://example.test/")
        .page(1, &serde_json::from_str(DESKTOP).expect("fixture"));
    host
}

#[test]
fn the_panel_command_opens_the_panel() {
    let answer = ResponsiveImages
        .run_command(&mut host(), "panel", "")
        .expect("answered");
    let answer: Value = serde_json::from_str(&answer).expect("json");
    assert_eq!(answer, json!({ OPEN_KEY: { "panel": PANEL_ID } }));
}

#[test]
fn the_check_command_and_the_tools_answer() {
    let mut host = host();
    let checked: Value = serde_json::from_str(
        &ResponsiveImages
            .run_command(&mut host, "check", "{}")
            .expect("checked"),
    )
    .expect("json");
    assert_eq!(checked["findings"], 4);
    let tool: Value = serde_json::from_str(
        &ResponsiveImages
            .call_tool(&mut host, "check", r#"{"target": 1}"#)
            .expect("tool"),
    )
    .expect("json");
    assert_eq!(tool["views"][0]["images"].as_array().map(Vec::len), Some(5));
    let cleared: Value = serde_json::from_str(
        &ResponsiveImages
            .call_tool(&mut host, "mark", r#"{"clear": true}"#)
            .expect("mark"),
    )
    .expect("json");
    assert_eq!(cleared["cleared"], true);
}

#[test]
fn an_unknown_command_or_tool_is_not_found() {
    for failure in [
        ResponsiveImages
            .run_command(&mut host(), "nothing", "")
            .expect_err("command"),
        ResponsiveImages
            .call_tool(&mut host(), "nothing", "")
            .expect_err("tool"),
    ] {
        assert_eq!(failure.code(), Some(codes::UNKNOWN_CONTRIBUTION));
    }
}

#[test]
fn the_check_runs_for_checks_run_and_describes_its_rules() {
    let mut plugin = ResponsiveImages;
    assert_eq!(plugin.describe().len(), rules::RULES.len());
    let ctx = CheckContext {
        view: None,
        url: None,
        rules: Vec::new(),
        args: "{}".to_owned(),
    };
    let found = plugin.run(&mut host(), &ctx).expect("run");
    assert_eq!(found.len(), 4);
    assert_eq!(key("panel.title"), "plugin.responsive-images.panel.title");
}
