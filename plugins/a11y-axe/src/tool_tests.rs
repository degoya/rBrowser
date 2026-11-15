use rb_plugin_api::{FailureKind, codes};
use serde_json::{Value, json};

use super::*;
use crate::fake::FakeHost;

fn three_views() -> FakeHost {
    FakeHost::with_views(&[
        (1, "mobile", "narrow"),
        (2, "tablet", "narrow"),
        (3, "desktop", "wide"),
    ])
}

fn answer(host: &mut FakeHost, args: &str, report: bool) -> Value {
    let text = check(host, args, report).expect("the check answers");
    serde_json::from_str(&text).expect("JSON")
}

fn entry<'a>(answer: &'a Value, rule: &str) -> &'a Value {
    answer["findings"]
        .as_array()
        .and_then(|list| list.iter().find(|entry| entry["rule"] == rule))
        .expect("the rule's entry")
}

#[test]
fn findings_are_merged_across_views_with_their_breakpoints() {
    let mut host = three_views();
    let answer = answer(&mut host, r#"{"target": "all"}"#, false);
    assert_eq!(answer["axeVersion"], "4.14.0");
    assert_eq!(answer["tags"], json!(["wcag22aa"]));
    assert_eq!(answer["rules"], 64);
    assert_eq!(answer["findings"].as_array().map(Vec::len), Some(2));
    let button = entry(&answer, "button-name");
    assert_eq!(button["views"], json!([1, 2]));
    assert_eq!(button["breakpoints"], json!(["mobile", "tablet"]));
    assert_eq!(button["severity"], "error");
    assert_eq!(button["help"], "Buttons must have discernible text");
    assert_eq!(button["path"], "html > body > header > button");
    let image = entry(&answer, "image-alt");
    assert_eq!(image["views"], json!([1, 2, 3]));
    assert_eq!(image["sharedId"], "img-alt-missing");
    assert_eq!(answer["views"][2]["findings"], 1);
    assert_eq!(answer["views"][0]["breakpoint"], "mobile");
    assert_eq!(answer["truncated"], false);
}

#[test]
fn a_target_view_runs_that_view_alone() {
    let mut host = three_views();
    let answer = answer(&mut host, r#"{"target": 3}"#, false);
    assert_eq!(host.evaluated.len(), 1);
    assert_eq!(host.evaluated[0].0, 3);
    assert!(
        answer["findings"]
            .as_array()
            .is_some_and(|list| list.iter().all(|entry| entry["views"] == json!([3])))
    );
}

#[test]
fn without_arguments_every_view_runs() {
    let mut host = three_views();
    answer(&mut host, "", false);
    assert_eq!(host.evaluated.len(), 3);
}

#[test]
fn tags_narrow_the_rules_for_this_call() {
    let mut host = three_views();
    let answer = answer(&mut host, r#"{"tags": ["wcag2a"]}"#, false);
    assert_eq!(answer["rules"], 57);
    assert_eq!(answer["tags"], json!(["wcag2a"]));
}

#[test]
fn bad_arguments_are_refused() {
    let mut host = three_views();
    for args in [
        r#"{"target": "mobile-ish"}"#,
        r#"{"target": -1}"#,
        r#"{"tags": ["a b"]}"#,
        r#"{"other": 1}"#,
        "[",
    ] {
        let failure = check(&mut host, args, false).expect_err(args);
        assert_eq!(failure.kind, FailureKind::BadData, "{args}");
        assert_eq!(failure.code(), Some("plugin.a11y-axe.errors.bad_args"));
    }
    assert!(host.evaluated.is_empty());
}

#[test]
fn a_failing_view_is_named_and_the_others_answer() {
    let mut host = three_views();
    host.answers.insert(
        3,
        Err(Failure::coded(
            FailureKind::NotSupported,
            codes::NOT_AVAILABLE,
        )),
    );
    let answer = answer(&mut host, "{}", false);
    assert_eq!(answer["views"][2]["error"], codes::NOT_AVAILABLE);
    assert_eq!(entry(&answer, "image-alt")["views"], json!([1, 2]));
}

#[test]
fn every_view_failing_fails_the_call() {
    let mut host = three_views();
    for view in [1, 2, 3] {
        host.answers.insert(
            view,
            Err(Failure::coded(
                FailureKind::NotSupported,
                codes::NOT_AVAILABLE,
            )),
        );
    }
    let failure = check(&mut host, "{}", false).expect_err("no view ran");
    assert_eq!(failure.code(), Some(codes::NOT_AVAILABLE));
}

#[test]
fn the_command_reports_the_findings_of_every_view() {
    let mut host = three_views();
    answer(&mut host, "{}", true);
    assert_eq!(host.reported.len(), 5);
    let mut tool_host = three_views();
    answer(&mut tool_host, "{}", false);
    assert!(tool_host.reported.is_empty());
}

#[test]
fn the_command_answers_when_the_findings_list_is_not_wired() {
    let mut host = three_views();
    host.findings_wired = false;
    let answer = answer(&mut host, "{}", true);
    assert_eq!(answer["findings"].as_array().map(Vec::len), Some(2));
    assert_eq!(host.logs.len(), 1);
}

#[test]
fn the_command_reports_at_most_the_hosts_limit() {
    let mut host = three_views();
    let node = json!({ "selector": "img", "path": "html > body > img", "impact": "minor" });
    let nodes: Vec<Value> = (0..4_000).map(|_| node.clone()).collect();
    let big = json!({
        "axeVersion": "4.14.0",
        "url": "http://localhost/",
        "violations": [{ "id": "image-alt", "impact": "minor", "nodes": nodes }],
    })
    .to_string();
    for view in [1, 2, 3] {
        host.answers.insert(view, Ok(big.clone()));
    }
    answer(&mut host, "{}", true);
    assert_eq!(host.reported.len(), MAX_FINDINGS);
}
