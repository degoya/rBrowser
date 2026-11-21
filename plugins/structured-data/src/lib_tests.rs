use rb_plugin_api::check::Check;
use rb_plugin_api::codes;
use rb_plugin_api::feature::Feature;
use serde_json::Value;

use super::*;
use crate::testing::{PAGE, URL, host, view};

#[test]
fn the_panel_command_opens_the_panel() {
    let mut host = host(PAGE);
    let answer = StructuredData
        .run_command(&mut host, "panel", "")
        .expect("answer");
    let value: Value = serde_json::from_str(&answer).expect("json");
    assert_eq!(value[OPEN_KEY]["panel"], PANEL_ID);
}

#[test]
fn the_tool_and_the_check_answer_through_the_traits() {
    let mut host = host(PAGE);
    view(&mut host, 1, URL);
    let answer = StructuredData
        .call_tool(&mut host, "inspect", r#"{"target": 1}"#)
        .expect("answer");
    let value: Value = serde_json::from_str(&answer).expect("json");
    assert_eq!(value["findings"][0]["property"], "headlinee");
    let ctx = CheckContext {
        view: Some(1),
        args: "{}".to_owned(),
        ..CheckContext::default()
    };
    assert_eq!(
        StructuredData.run(&mut host, &ctx).expect("findings").len(),
        1
    );
    assert_eq!(StructuredData.describe().len(), rules::RULES.len());
}

#[test]
fn an_unknown_tool_or_command_is_named() {
    let mut host = host(PAGE);
    let failure = StructuredData
        .call_tool(&mut host, "nope", "{}")
        .expect_err("unknown");
    assert_eq!(failure.code(), Some(codes::UNKNOWN_CONTRIBUTION));
    let failure = StructuredData
        .run_command(&mut host, "nope", "{}")
        .expect_err("unknown");
    assert_eq!(failure.code(), Some(codes::UNKNOWN_CONTRIBUTION));
}

#[test]
fn the_check_command_from_the_context_menu_checks_the_clicked_view() {
    let mut host = host(PAGE);
    view(&mut host, 1, URL);
    view(&mut host, 2, URL);
    host.active = Some(1);
    let answer = StructuredData
        .run_command(&mut host, "check", r#"{"target": 2}"#)
        .expect("answer");
    let value: Value = serde_json::from_str(&answer).expect("json");
    assert_eq!(value["view"], 2);
    assert_eq!(host.findings[0].view, Some(2));
}
