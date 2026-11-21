use rb_plugin_api::Severity;
use serde_json::{Value, json};

use super::{Answer, Message, parse};
use crate::rules;
use crate::settings::{Engine, Settings, Source};

const PAGE: &str = "http://localhost:5173/menu.html";

fn every() -> Vec<String> {
    rules::all().iter().map(|rule| rule.id.clone()).collect()
}

fn message(rule: &str, severity: Severity, line: u32, column: u32) -> Message {
    Message {
        rule: rule.to_owned(),
        severity,
        message: format!("{rule} here"),
        line,
        column,
        selector: Some("#intro".to_owned()),
        extract: "<p id=\"intro\">".to_owned(),
    }
}

fn answer(messages: Vec<Message>) -> Answer {
    Answer {
        version: "11.18.0".to_owned(),
        messages,
        ..Answer::default()
    }
}

#[test]
fn parses_the_runner_answer_as_object_and_as_string() {
    let object = json!({
        "engine": "builtin", "version": "11.18.0", "truncated": false,
        "messages": [{ "rule": "no-dup-id", "severity": "error", "message": "Duplicate ID",
            "line": 10, "column": 4, "selector": "#intro", "extract": "id=\"intro\"" }],
    });
    for text in [
        object.to_string(),
        Value::String(object.to_string()).to_string(),
    ] {
        let parsed = parse(1, &text, Engine::Builtin, "").expect("answer");
        assert_eq!(parsed.messages.len(), 1);
        assert_eq!(parsed.messages[0].line, 10);
        assert_eq!(parsed.messages[0].severity, Severity::Error);
    }
}

#[test]
fn a_failed_nu_validation_names_the_address_and_the_reason() {
    let text = json!({ "engine": "nu", "error": "Failed to fetch" }).to_string();
    let failure = parse(2, &text, Engine::Nu, "http://localhost:8888/").expect_err("nu failed");
    assert_eq!(
        failure.code(),
        Some("plugin.html-validate.errors.nu_failed")
    );
    assert_eq!(failure.param("address"), Some("http://localhost:8888/"));
    assert_eq!(failure.param("view"), Some("2"));
    assert_eq!(failure.param("reason"), Some("Failed to fetch"));
}

#[test]
fn a_blocked_nu_request_says_what_can_refuse_it() {
    let text = json!({ "engine": "nu", "error": "Failed to fetch", "blocked": true }).to_string();
    let failure = parse(2, &text, Engine::Nu, "http://localhost:8888/").expect_err("blocked");
    assert_eq!(
        failure.code(),
        Some("plugin.html-validate.errors.nu_blocked")
    );
    assert_eq!(failure.param("address"), Some("http://localhost:8888/"));
    assert_eq!(failure.param("view"), Some("2"));
    assert_eq!(failure.param("reason"), Some("Failed to fetch"));
}

#[test]
fn a_failed_builtin_validation_is_its_own_failure() {
    let text = json!({ "engine": "builtin", "error": "boom" }).to_string();
    let failure = parse(2, &text, Engine::Builtin, "").expect_err("failed");
    assert_eq!(
        failure.code(),
        Some("plugin.html-validate.errors.validate_failed")
    );
}

#[test]
fn an_answer_that_does_not_read_is_a_bad_answer() {
    for text in ["", "[1]", "\"x\"", "{\"messages\": 4}"] {
        let failure = parse(1, text, Engine::Builtin, "").expect_err(text);
        assert_eq!(
            failure.code(),
            Some("plugin.html-validate.errors.bad_answer"),
            "{text}"
        );
    }
}

#[test]
fn a_message_becomes_a_finding_with_line_column_and_extract() {
    let found = answer(vec![message("no-dup-id", Severity::Error, 10, 4)]).findings(
        3,
        PAGE,
        Engine::Builtin,
        Source::Raw,
        &Settings::default(),
        &every(),
    );
    assert_eq!(found.len(), 1);
    let finding = &found[0];
    assert_eq!(finding.rule, "no-dup-id");
    assert_eq!(finding.severity, Severity::Error);
    assert_eq!(finding.message, "plugin.html-validate.findings.at");
    let param = |name: &str| {
        finding
            .params
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    };
    assert_eq!(param("line"), Some("10"));
    assert_eq!(param("column"), Some("4"));
    assert_eq!(param("message"), Some("no-dup-id here"));
    assert_eq!(finding.view, Some(3));
    assert_eq!(finding.url.as_deref(), Some(PAGE));
    assert_eq!(finding.selector.as_deref(), Some("#intro"));
    assert_eq!(finding.path.as_deref(), Some("source:10:4"));
    let data: Value = serde_json::from_str(finding.data.as_deref().expect("data")).expect("json");
    assert_eq!(data["extract"], json!("<p id=\"intro\">"));
    assert_eq!(data["engine"], json!("builtin"));
    assert_eq!(
        data["helpUrl"],
        json!("https://html-validate.org/rules/no-dup-id.html")
    );
}

#[test]
fn a_message_without_a_place_is_a_document_finding() {
    let mut nu = message("nu-error", Severity::Error, 0, 0);
    nu.selector = None;
    let found = answer(vec![nu]).findings(
        1,
        PAGE,
        Engine::Nu,
        Source::Raw,
        &Settings::default(),
        &every(),
    );
    assert_eq!(found[0].message, "plugin.html-validate.findings.document");
    assert!(found[0].params.iter().all(|(key, _)| key != "line"));
    assert_eq!(found[0].selector, None);
    let data: Value = serde_json::from_str(found[0].data.as_deref().expect("data")).expect("json");
    assert_eq!(data["helpUrl"], Value::Null);
}

#[test]
fn two_messages_of_one_rule_at_one_place_stay_two_findings() {
    let found = answer(vec![
        message("nu-error", Severity::Error, 11, 1),
        message("nu-error", Severity::Error, 11, 1),
        message("nu-error", Severity::Error, 12, 1),
    ])
    .findings(
        1,
        PAGE,
        Engine::Nu,
        Source::Raw,
        &Settings::default(),
        &every(),
    );
    let paths: Vec<&str> = found.iter().filter_map(|f| f.path.as_deref()).collect();
    assert_eq!(paths, ["source:11:1", "source:11:1#2", "source:12:1"]);
}

#[test]
fn keeps_only_wanted_rules_at_or_above_the_lowest_severity() {
    let settings = Settings {
        disabled_rules: vec!["close-order".to_owned()],
        ..Settings::default()
    };
    let enabled: Vec<String> = every().into_iter().filter(|id| id != "no-dup-id").collect();
    let found = answer(vec![
        message("no-dup-id", Severity::Error, 1, 1),
        message("close-order", Severity::Error, 2, 1),
        message("nu-warning", Severity::Warning, 3, 1),
        message("nu-info", Severity::Info, 4, 1),
        message("unknown-rule", Severity::Error, 5, 1),
    ])
    .findings(1, PAGE, Engine::Nu, Source::Raw, &settings, &enabled);
    let rules: Vec<&str> = found.iter().map(|f| f.rule.as_str()).collect();
    assert_eq!(rules, ["nu-warning"]);
}

// RB-094-06: findings of the rendered DOM name their element, not a line of the serialization.

fn param<'a>(finding: &'a rb_plugin_api::Finding, name: &str) -> Option<&'a str> {
    finding
        .params
        .iter()
        .find(|(key, _)| key == name)
        .map(|(_, value)| value.as_str())
}

#[test]
fn a_rendered_message_is_a_finding_of_its_element() {
    let found = answer(vec![message("no-dup-id", Severity::Error, 10, 4)]).findings(
        2,
        PAGE,
        Engine::Builtin,
        Source::Rendered,
        &Settings::default(),
        &every(),
    );
    let finding = &found[0];
    assert_eq!(finding.message, "plugin.html-validate.findings.element");
    assert_eq!(param(finding, "selector"), Some("#intro"));
    assert_eq!(param(finding, "line"), None);
    assert_eq!(finding.selector.as_deref(), Some("#intro"));
    assert_eq!(finding.path.as_deref(), Some("#intro"));
    let data: Value = serde_json::from_str(finding.data.as_deref().expect("data")).expect("json");
    assert_eq!(data["source"], json!("rendered"));
    assert_eq!(data["line"], Value::Null);
    assert_eq!(data["column"], Value::Null);
}

#[test]
fn a_rendered_message_without_an_element_is_a_document_finding_without_a_path() {
    let mut doc = message("missing-doctype", Severity::Error, 1, 1);
    doc.selector = None;
    let found = answer(vec![doc]).findings(
        2,
        PAGE,
        Engine::Builtin,
        Source::Rendered,
        &Settings::default(),
        &every(),
    );
    assert_eq!(found[0].message, "plugin.html-validate.findings.document");
    assert_eq!(found[0].selector, None);
    assert_eq!(found[0].path, None);
}

#[test]
fn two_rendered_messages_of_one_rule_at_one_element_stay_two_findings() {
    let found = answer(vec![
        message("no-dup-id", Severity::Error, 3, 1),
        message("no-dup-id", Severity::Error, 9, 1),
    ])
    .findings(
        1,
        PAGE,
        Engine::Builtin,
        Source::Rendered,
        &Settings::default(),
        &every(),
    );
    let paths: Vec<&str> = found.iter().filter_map(|f| f.path.as_deref()).collect();
    assert_eq!(paths, ["#intro", "#intro#2"]);
    assert!(
        found
            .iter()
            .all(|f| f.selector.as_deref() == Some("#intro"))
    );
}

#[test]
fn a_raw_finding_says_its_source() {
    let found = answer(vec![message("no-dup-id", Severity::Error, 10, 4)]).findings(
        2,
        PAGE,
        Engine::Builtin,
        Source::Raw,
        &Settings::default(),
        &every(),
    );
    let data: Value = serde_json::from_str(found[0].data.as_deref().expect("data")).expect("json");
    assert_eq!(data["source"], json!("raw"));
    assert_eq!(data["line"], json!(10));
}
