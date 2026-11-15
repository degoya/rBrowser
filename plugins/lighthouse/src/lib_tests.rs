use rb_plugin_api::check::Check;
use rb_plugin_api::feature::Feature;
use rb_plugin_api::{CheckContext, codes};
use serde_json::Value;

use super::*;
use crate::fake::FakeHost;

#[test]
fn describes_one_rule_per_audit() {
    let rules = Lighthouse.describe();
    assert_eq!(rules.len(), rules::all().len());
    assert!(
        rules
            .iter()
            .any(|rule| rule.id == "largest-contentful-paint")
    );
}

#[test]
fn the_command_and_the_tool_run_and_answer_the_summary() {
    let mut host = FakeHost::with_views(&[(1, "mobile", 375)]).done().done();
    let answer = Lighthouse
        .call_tool(&mut host, "run", r#"{"categories":["accessibility"]}"#)
        .expect("runs");
    let summary: Value = serde_json::from_str(&answer).expect("JSON");
    assert_eq!(summary["breakpoints"][0]["scores"]["accessibility"], 62);
    assert!(host.reported.is_empty());
    Lighthouse
        .run_command(&mut host, "run", "{}")
        .expect("the command runs");
    assert!(!host.reported.is_empty());
}

#[test]
fn the_check_world_runs_the_contexts_view() {
    let mut host = FakeHost::with_views(&[(1, "mobile", 375)]).done();
    let ctx = CheckContext {
        view: Some(1),
        url: None,
        rules: vec!["color-contrast".to_owned()],
        args: String::new(),
    };
    let findings = Lighthouse.run(&mut host, &ctx).expect("checks");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].rule, "color-contrast");
}

#[test]
fn unknown_commands_and_tools_are_refused() {
    let mut host = FakeHost::default();
    let failure = Lighthouse
        .run_command(&mut host, "nope", "{}")
        .expect_err("unknown");
    assert_eq!(failure.code(), Some(codes::UNKNOWN_CONTRIBUTION));
    assert!(Lighthouse.call_tool(&mut host, "nope", "{}").is_err());
}

#[test]
fn every_key_the_code_uses_is_in_the_locale() {
    let locale: Value =
        serde_json::from_str(include_str!("../locales/en.json")).expect("the locale reads");
    let plugin = &locale["plugin"]["lighthouse"];
    for (group, name) in [
        ("errors", "bad_args"),
        ("errors", "browser_missing"),
        ("errors", "no_browser"),
        ("errors", "no_sidecar"),
        ("errors", "no_targets"),
        ("errors", "sidecar_failed"),
        ("errors", "timed_out"),
        ("findings", "audit"),
        ("findings", "audit_value"),
        ("findings", "element"),
        ("commands", "run"),
        ("tools", "run"),
        ("tools", "status"),
        ("tools", "cancel"),
        ("panels", "lighthouse"),
    ] {
        assert!(plugin[group][name].is_string(), "{group}.{name}");
    }
    assert!(plugin["settings"]["throttling"]["label"].is_string());
}
