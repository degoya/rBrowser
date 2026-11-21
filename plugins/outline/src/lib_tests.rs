use rb_plugin_api::check::Check;
use rb_plugin_api::feature::Feature;
use serde_json::{Value, json};

use super::*;
use crate::fake::FakeHost;

#[test]
fn the_panel_command_asks_the_host_to_open_the_panel() {
    let mut host = FakeHost::default();
    let answer = Outline
        .run_command(&mut host, "panel", "{}")
        .expect("answer");
    let value: Value = serde_json::from_str(&answer).expect("json");
    assert_eq!(value, json!({ "$open": { "panel": "outline" } }));
}

#[test]
fn unknown_commands_and_tools_are_refused() {
    let mut host = FakeHost::default();
    for failure in [
        Outline
            .run_command(&mut host, "nope", "{}")
            .expect_err("command"),
        Outline
            .call_tool(&mut host, "nope", "{}")
            .expect_err("tool"),
        Outline
            .call_tool(&mut host, "@page", "{}")
            .expect_err("no page script"),
    ] {
        assert_eq!(failure.code(), Some(codes::UNKNOWN_CONTRIBUTION));
    }
}

#[test]
fn the_tools_answer_json() {
    let mut host = FakeHost::default();
    host.view(1, "Desktop", "https://site.test/")
        .page(1, &json!({ "url": "https://site.test/" }));
    let answer = Outline
        .call_tool(&mut host, "get", r#"{"sections":["headings"]}"#)
        .expect("answer");
    let value: Value = serde_json::from_str(&answer).expect("json");
    assert!(value["sections"]["headings"]["rows"].is_array());
}

#[test]
fn describe_lists_the_rules() {
    assert_eq!(Outline.describe().len(), rules::RULES.len());
}

#[test]
fn the_breakpoint_commands_and_tools_are_served() {
    let mut host = FakeHost::default();
    host.view(1, "Desktop", "https://site.test/")
        .page(1, &json!({ "url": "https://site.test/" }));
    for command in ["breakpoint_diff", "reading_order"] {
        let answer = Outline
            .run_command(&mut host, command, "{}")
            .expect("command");
        let value: Value = serde_json::from_str(&answer).expect("json");
        assert_eq!(value["$open"], json!({ "panel": "outline" }));
        let answer = Outline.call_tool(&mut host, command, "{}").expect("tool");
        let value: Value = serde_json::from_str(&answer).expect("json");
        assert!(value["findings"].is_array());
    }
}
