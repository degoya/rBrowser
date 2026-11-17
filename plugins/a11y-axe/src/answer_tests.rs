use rb_plugin_api::{FailureKind, Severity};
use serde_json::Value;

use super::*;
use crate::fake::fixture;

fn ids(list: &[&str]) -> Vec<String> {
    list.iter().map(|id| (*id).to_owned()).collect()
}

#[test]
fn a_stored_answer_reads_and_maps_each_node_to_a_finding() {
    let answer = parse(2, &fixture("narrow")).expect("answer");
    assert_eq!(answer.axe_version, "4.14.0");
    let found = answer.findings(2, &ids(&["button-name", "image-alt"]), false);
    assert_eq!(found.len(), 2);
    let button = &found[0];
    assert_eq!(button.rule, "button-name");
    assert_eq!(button.severity, Severity::Error);
    assert_eq!(button.message, "plugin.a11y-axe.findings.violation");
    assert_eq!(button.view, Some(2));
    assert_eq!(
        button.url.as_deref(),
        Some("http://localhost:5173/a11y.html#top")
    );
    assert_eq!(button.selector.as_deref(), Some(".burger"));
    assert_eq!(
        button.path.as_deref(),
        Some("html > body > header > button")
    );
    assert!(button.params.contains(&(
        "help".to_owned(),
        "Buttons must have discernible text".to_owned()
    )));
    assert!(
        button
            .params
            .contains(&("impact".to_owned(), "critical".to_owned()))
    );
    let data: Value = serde_json::from_str(button.data.as_deref().expect("data")).expect("json");
    assert_eq!(data["wcag"], serde_json::json!(["wcag2a", "wcag412"]));
    assert_eq!(
        data["helpUrl"],
        "https://dequeuniversity.com/rules/axe/4.14/button-name?application=axeAPI"
    );
    assert_eq!(data["needsReview"], false);
    assert!(
        data["summary"]
            .as_str()
            .is_some_and(|s| s.contains("inner text"))
    );
}

#[test]
fn needs_review_results_are_information_and_only_when_asked() {
    let answer = parse(1, &fixture("wide")).expect("answer");
    let all = ids(&["image-alt", "color-contrast"]);
    assert_eq!(answer.findings(1, &all, false).len(), 1);
    let found = answer.findings(1, &all, true);
    assert_eq!(found.len(), 2);
    let review = &found[1];
    assert_eq!(review.rule, "color-contrast");
    assert_eq!(review.severity, Severity::Info);
    assert_eq!(review.message, "plugin.a11y-axe.findings.incomplete");
}

#[test]
fn rules_not_asked_for_are_left_out() {
    let answer = parse(1, &fixture("narrow")).expect("answer");
    let found = answer.findings(1, &ids(&["image-alt"]), false);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].rule, "image-alt");
}

#[test]
fn an_answer_given_as_a_json_string_reads_too() {
    let text = serde_json::to_string(&fixture("narrow")).expect("json");
    assert_eq!(parse(1, &text).expect("answer").violations.len(), 2);
}

#[test]
fn axe_failing_in_the_view_is_a_failure_naming_view_and_reason() {
    let failure = parse(3, &fixture("failed")).expect_err("failed");
    assert_eq!(failure.code(), Some("plugin.a11y-axe.errors.axe_failed"));
    assert_eq!(failure.param("view"), Some("3"));
    assert_eq!(
        failure.param("reason"),
        Some("'##' is not a valid selector")
    );
}

#[test]
fn an_unreadable_answer_is_bad_data() {
    for text in ["not json", "42", "[1]", r#"{"violations": "no"}"#] {
        let failure = parse(1, text).expect_err(text);
        assert_eq!(failure.kind, FailureKind::BadData, "{text}");
        assert_eq!(failure.code(), Some("plugin.a11y-axe.errors.bad_answer"));
    }
}
