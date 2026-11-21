use rb_plugin_api::check::Check;
use rb_plugin_api::feature::Feature;
use rb_plugin_api::{CheckContext, codes};
use serde_json::Value;

use super::*;
use crate::testing::{self, SECRET};

#[test]
fn describe_lists_every_rule_with_a_title_key() {
    let rules = ConsentCheck.describe();
    let ids: Vec<&str> = rules.iter().map(|rule| rule.id.as_str()).collect();
    assert_eq!(
        ids,
        vec![
            "request.before_consent",
            "cookie.before_consent",
            "request.after_reject",
            "cookie.after_reject"
        ]
    );
    assert!(
        rules
            .iter()
            .all(|rule| rule.title.starts_with("plugin.consent-check.rules."))
    );
}

#[test]
fn a_checks_run_judges_the_probe_capture_with_the_enabled_rules() {
    let mut host = testing::host();
    let ctx = CheckContext {
        view: Some(1),
        url: Some(testing::PAGE.to_owned()),
        rules: vec!["cookie.before_consent".to_owned()],
        args: testing::probe_args(false).to_string(),
    };
    let findings = ConsentCheck.run(&mut host, &ctx).expect("findings");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].rule, "cookie.before_consent");
    assert!(
        !serde_json::to_string(&findings)
            .expect("JSON")
            .contains(SECRET)
    );
}

#[test]
fn a_checks_run_without_a_probe_reads_its_view() {
    let mut host = testing::host();
    let ctx = CheckContext {
        view: Some(1),
        url: None,
        rules: Vec::new(),
        args: testing::feed_args(1).to_string(),
    };
    let findings = ConsentCheck.run(&mut host, &ctx).expect("findings");
    assert_eq!(findings.len(), 2);
    assert!(findings.iter().all(|finding| finding.view == Some(1)));
}

#[test]
fn the_tool_and_the_command_answer_json() {
    let mut host = testing::host();
    let answer: Value = serde_json::from_str(
        &ConsentCheck
            .call_tool(&mut host, "check", "{}")
            .expect("an answer"),
    )
    .expect("JSON");
    assert_eq!(answer["mode"], "view");
    let answer: Value = serde_json::from_str(
        &ConsentCheck
            .run_command(&mut host, "check", &testing::feed_args(1).to_string())
            .expect("an answer"),
    )
    .expect("JSON");
    assert_eq!(answer["findings"], 2);
}

#[test]
fn unknown_names_and_malformed_arguments_are_refused() {
    let mut host = testing::host();
    let unknown = ConsentCheck
        .call_tool(&mut host, "inspect", "{}")
        .expect_err("no such tool");
    assert_eq!(unknown.code(), Some(codes::UNKNOWN_CONTRIBUTION));
    let bad = ConsentCheck
        .call_tool(&mut host, "check", "[1]")
        .expect_err("no object");
    assert_eq!(bad.code(), Some(codes::INVALID));
    assert!(ConsentCheck.run_command(&mut host, "panel", "{}").is_err());
}

#[test]
fn a_checks_run_reads_its_view_even_with_an_offer_in_its_arguments() {
    let mut host = testing::host();
    let mut args = testing::feed_args(1);
    args[tool::ASK_KEY] = Value::Bool(true);
    let ctx = CheckContext {
        view: Some(1),
        url: None,
        rules: Vec::new(),
        args: args.to_string(),
    };
    let findings = ConsentCheck.run(&mut host, &ctx).expect("findings");
    assert_eq!(findings.len(), 2, "no probe request from a checks run");
}
