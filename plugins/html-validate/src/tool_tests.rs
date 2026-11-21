use serde_json::{Value, json};

use super::validate;
use crate::fake::{FakeHost, PAGE, fixture};

fn host() -> FakeHost {
    FakeHost::with_views(
        &[(1, "mobile"), (2, "desktop")],
        &fixture("invalid.html"),
        &fixture("answer-invalid.json"),
    )
}

fn answer(host: &mut FakeHost, args: &str, report: bool) -> Value {
    let text = validate(host, args, report).expect("answer");
    serde_json::from_str(&text).expect("json")
}

#[test]
fn merges_the_messages_of_every_view_with_their_breakpoints() {
    let mut host = host();
    let value = answer(&mut host, "", false);
    assert_eq!(value["engine"], json!("builtin"));
    assert_eq!(value["htmlValidate"], json!("11.18.0"));
    assert_eq!(value["truncated"], json!(false));
    let findings = value["findings"].as_array().expect("findings");
    assert_eq!(findings.len(), 3);
    let dup = findings
        .iter()
        .find(|f| f["rule"] == json!("no-dup-id"))
        .expect("the duplicate id");
    assert_eq!(dup["line"], json!(10));
    assert_eq!(dup["severity"], json!("error"));
    assert_eq!(dup["url"], json!(PAGE));
    assert_eq!(dup["views"], json!([1, 2]));
    assert_eq!(dup["breakpoints"], json!(["mobile", "desktop"]));
    assert_eq!(dup["extract"], json!("<p id=\"intro\">Second</p>"));
    let nested = findings
        .iter()
        .find(|f| f["rule"] == json!("element-permitted-content"))
        .expect("the misnested element");
    assert_eq!(nested["line"], json!(12));
    assert_eq!(value["errors"], json!(3));
}

#[test]
fn views_with_the_same_source_share_one_validation() {
    let mut host = host();
    answer(&mut host, "", false);
    assert_eq!(host.evaluated.len(), 1);
    assert_eq!(host.progress, [(1, 2), (2, 2)]);
}

#[test]
fn the_command_reports_into_the_findings_panel_and_the_tool_does_not() {
    let mut host = host();
    answer(&mut host, "", false);
    assert!(host.reported.is_empty());
    answer(&mut host, "", true);
    assert_eq!(
        host.reported.len(),
        6,
        "three findings in each of two views"
    );
}

#[test]
fn target_names_one_view() {
    let mut host = host();
    let value = answer(&mut host, r#"{"target": 2}"#, false);
    assert_eq!(value["views"].as_array().map(Vec::len), Some(1));
    assert_eq!(value["findings"][0]["views"], json!([2]));
}

#[test]
fn the_engine_argument_overrides_the_setting() {
    let mut host = host().set("nuAddress", "\"http://localhost:8888\"");
    host.answer = Some(fixture("answer-nu.json"));
    let value = answer(&mut host, r#"{"engine": "nu"}"#, false);
    assert_eq!(value["engine"], json!("nu"));
    let rules: Vec<&Value> = value["findings"]
        .as_array()
        .expect("findings")
        .iter()
        .map(|f| &f["rule"])
        .collect();
    // nu-info is below the default lowest severity, warning.
    assert_eq!(rules, [&json!("nu-error"), &json!("nu-error")]);
    let script = &host.evaluated[0].1;
    assert!(script.contains(r#""endpoint":"http://localhost:8888/?out=json""#));
}

#[test]
fn the_nu_engine_without_an_address_fails_before_any_view() {
    let mut host = host();
    let failure = validate(&mut host, r#"{"engine": "nu"}"#, false).expect_err("no address");
    assert_eq!(
        failure.code(),
        Some("plugin.html-validate.errors.nu_address_missing")
    );
    assert!(host.cdp.is_empty());
}

#[test]
fn a_failing_nu_address_fails_the_run_with_its_reason() {
    let mut host = host()
        .set("engine", "\"nu\"")
        .set("nuAddress", "\"http://localhost:8888/\"");
    host.answer = Some(json!({ "engine": "nu", "error": "Failed to fetch" }).to_string());
    let failure = validate(&mut host, "", false).expect_err("unreachable");
    assert_eq!(
        failure.code(),
        Some("plugin.html-validate.errors.nu_failed")
    );
    assert_eq!(failure.param("reason"), Some("Failed to fetch"));
    assert_eq!(failure.param("address"), Some("http://localhost:8888/"));
    // One request for the shared source, never a fallback to html-validate.
    assert_eq!(host.evaluated.len(), 1);
}

#[test]
fn a_view_without_cdp_is_listed_with_its_error_and_the_others_still_count() {
    let mut host = host();
    host.sources.remove(&2);
    let value = answer(&mut host, "", false);
    let views = value["views"].as_array().expect("views");
    assert_eq!(views[0]["findings"], json!(3));
    assert_eq!(views[1]["error"], json!("plugins.error.not_available"));
    assert_eq!(value["findings"].as_array().map(Vec::len), Some(3));
}

#[test]
fn unknown_arguments_and_targets_are_refused() {
    let mut host = host();
    for args in [
        r#"{"tags": []}"#,
        r#"{"target": "mobile"}"#,
        r#"{"engine": "w3c"}"#,
        r#"{"source": "dom"}"#,
        "[",
    ] {
        let failure = validate(&mut host, args, false).expect_err(args);
        assert_eq!(
            failure.code(),
            Some("plugin.html-validate.errors.bad_args"),
            "{args}"
        );
    }
}

#[test]
fn a_disabled_rule_is_left_out_of_the_answer() {
    let mut host = host().set("disabledRules", r#"["no-dup-id"]"#);
    let value = answer(&mut host, "", false);
    let findings = value["findings"].as_array().expect("findings");
    assert!(findings.iter().all(|f| f["rule"] != json!("no-dup-id")));
    // The runner bundles html-validate's presets, which name the rule; only the call counts.
    let call = host.evaluated[0]
        .1
        .replace(crate::script::RUNNER_SOURCE, "");
    assert!(call.contains(r#""rules":{"#), "{call}");
    assert!(!call.contains("no-dup-id"), "{call}");
}

// RB-094-06: the rendered DOM as the source, the project's rule file.

fn rendered_host() -> FakeHost {
    let mut host = host();
    let html = json!({ "html": "<!DOCTYPE html>\n<html><body><p id=\"intro\"></p></body></html>" });
    for view in [1, 2] {
        host.rendered.insert(view, Ok(html.to_string()));
    }
    host
}

fn runner_call(host: &FakeHost) -> String {
    host.evaluated
        .iter()
        .map(|(_, script)| script.as_str())
        .find(|script| script.contains(crate::script::RUNNER_SOURCE))
        .expect("a validation")
        .replace(crate::script::RUNNER_SOURCE, "")
}

#[test]
fn the_source_argument_validates_the_rendered_dom_by_element() {
    let mut host = rendered_host();
    let value = answer(&mut host, r#"{"source": "rendered"}"#, false);
    assert_eq!(value["source"], json!("rendered"));
    // The frame tree for the URL; the served document is never asked for.
    assert!(
        host.cdp
            .iter()
            .all(|(_, method, _)| method == "Page.getFrameTree")
    );
    // Two render scripts, one validation of the DOM both views share.
    assert_eq!(host.evaluated.len(), 3);
    assert!(runner_call(&host).contains(r#"<p id=\"intro\">"#));
    let dup = value["findings"]
        .as_array()
        .expect("findings")
        .iter()
        .find(|f| f["rule"] == json!("no-dup-id"))
        .expect("the duplicate id")
        .clone();
    assert_eq!(
        dup["selector"],
        json!("html > body > main > p:nth-child(2)")
    );
    assert_eq!(dup["line"], Value::Null);
    assert_eq!(dup["views"], json!([1, 2]));
}

#[test]
fn the_source_setting_is_the_default_of_a_run() {
    let mut host = rendered_host().set("source", "\"rendered\"");
    let value = answer(&mut host, "", true);
    assert_eq!(value["source"], json!("rendered"));
    assert!(
        host.reported
            .iter()
            .all(|f| f.message != "plugin.html-validate.findings.at")
    );
    let value = answer(&mut host, r#"{"source": "raw"}"#, false);
    assert_eq!(value["source"], json!("raw"));
}

#[test]
fn a_trusted_projects_rule_file_takes_precedence_over_the_rules_off() {
    let mut host = host().set("disabledRules", r#"["no-dup-id"]"#);
    host.project = Some(Ok(
        br#"{"rules": {"no-dup-id": "warn", "close-order": "off"}}"#.to_vec(),
    ));
    host.trusted = true;
    let value = answer(&mut host, "", false);
    assert_eq!(value["projectRules"], json!("applied"));
    let call = runner_call(&host);
    assert!(call.contains(r#""no-dup-id":"warn""#), "{call}");
    assert!(!call.contains("close-order"), "{call}");
    let rules: Vec<&Value> = value["findings"]
        .as_array()
        .expect("findings")
        .iter()
        .map(|f| &f["rule"])
        .collect();
    assert!(rules.contains(&&json!("no-dup-id")));
    assert!(!rules.contains(&&json!("close-order")));
}

#[test]
fn an_untrusted_projects_rule_file_is_left_out() {
    let mut host = host();
    host.project = Some(Ok(br#"{"rules": {"no-dup-id": "off"}}"#.to_vec()));
    host.trusted = false;
    let value = answer(&mut host, "", false);
    assert_eq!(value["projectRules"], json!("untrusted"));
    assert!(runner_call(&host).contains("no-dup-id"));
}

#[test]
fn a_rule_file_that_loads_code_fails_the_run_before_any_view() {
    let mut host = host();
    host.project = Some(Ok(br#"{"plugins": ["./evil.js"], "rules": {}}"#.to_vec()));
    host.trusted = true;
    let failure = validate(&mut host, "", false).expect_err("plugins");
    assert_eq!(
        failure.code(),
        Some("plugin.html-validate.errors.project_rules")
    );
    assert!(host.cdp.is_empty() && host.evaluated.is_empty());
}

#[test]
fn without_a_rule_file_the_answer_says_none() {
    let mut host = host();
    let value = answer(&mut host, "", false);
    assert_eq!(value["projectRules"], json!("none"));
    assert_eq!(value["source"], json!("raw"));
}
