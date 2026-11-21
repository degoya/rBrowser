use rb_plugin_api::check::Check;
use rb_plugin_api::codes;
use rb_plugin_api::feature::Feature;
use serde_json::Value;

use super::*;
use crate::testing::{URL, good, host, serve, view};

#[test]
fn the_compare_command_opens_the_panel_at_the_comparison() {
    let mut host = host();
    let answer = SecurityHeaders
        .run_command(&mut host, "compare_environments", "")
        .expect("answer");
    let value: Value = serde_json::from_str(&answer).expect("json");
    assert_eq!(value[OPEN_KEY]["panel"], PANEL_ID);
    assert_eq!(value[POST_KEY][0]["message"]["type"], "compare");
}

#[test]
fn the_tools_and_the_check_answer_through_the_traits() {
    let mut host = host();
    serve(&mut host, URL, &good());
    view(&mut host, 1, URL);
    let answer = SecurityHeaders
        .call_tool(&mut host, "check", "{}")
        .expect("answer");
    let value: Value = serde_json::from_str(&answer).expect("json");
    assert_eq!(value["url"], URL);
    let ctx = CheckContext {
        view: Some(1),
        args: "{}".to_owned(),
        ..CheckContext::default()
    };
    assert!(
        SecurityHeaders
            .run(&mut host, &ctx)
            .expect("findings")
            .is_empty()
    );
    assert_eq!(SecurityHeaders.describe().len(), rules::RULES.len());
}

#[test]
fn unknown_tools_and_commands_and_bad_arguments_are_refused() {
    let mut host = host();
    let unknown = SecurityHeaders
        .call_tool(&mut host, "nope", "{}")
        .expect_err("unknown");
    assert_eq!(unknown.code(), Some(codes::UNKNOWN_CONTRIBUTION));
    let unknown = SecurityHeaders
        .run_command(&mut host, "nope", "{}")
        .expect_err("unknown");
    assert_eq!(unknown.code(), Some(codes::UNKNOWN_CONTRIBUTION));
    let bad = SecurityHeaders
        .call_tool(&mut host, "check", "[1]")
        .expect_err("bad");
    assert_eq!(bad.code(), Some(codes::INVALID));
}
