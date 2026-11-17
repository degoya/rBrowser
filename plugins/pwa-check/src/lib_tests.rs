use serde_json::json;

use super::*;
use crate::fake::FakeHost;

#[test]
fn the_panel_command_opens_the_panel() {
    let mut host = FakeHost::default();
    let answer = PwaCheck
        .run_command(&mut host, "panel", "{}")
        .expect("answer");
    assert_eq!(answer, json!({ "$open": { "panel": "pwa" } }).to_string());
}

#[test]
fn unknown_contributions_and_bad_arguments_are_refused() {
    let mut host = FakeHost::default();
    let failure = PwaCheck
        .call_tool(&mut host, "nope", "{}")
        .expect_err("unknown");
    assert_eq!(failure.code(), Some(codes::UNKNOWN_CONTRIBUTION));
    let failure = PwaCheck
        .run_command(&mut host, "nope", "{}")
        .expect_err("unknown");
    assert_eq!(failure.code(), Some(codes::UNKNOWN_CONTRIBUTION));
    let failure = PwaCheck
        .call_tool(&mut host, "check", "[]")
        .expect_err("args");
    assert_eq!(failure.code(), Some(codes::INVALID));
}

#[test]
fn describe_lists_the_rules_and_a_check_run_answers_findings() {
    let mut host = FakeHost::default();
    host.view(1, "Desktop", "https://a.test/");
    host.state(&json!({ "url": "https://a.test/", "secure": false }));
    assert_eq!(PwaCheck.describe().len(), rules::RULES.len());
    let ctx = CheckContext {
        view: None,
        url: None,
        rules: Vec::new(),
        args: String::new(),
    };
    let findings = PwaCheck.run(&mut host, &ctx).expect("findings");
    assert!(findings.iter().any(|f| f.rule == rules::SW_INSECURE));
}
