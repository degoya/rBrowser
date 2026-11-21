use rb_plugin_api::check::Check;
use rb_plugin_api::feature::Feature;
use rb_plugin_api::{CheckContext, Severity};

use super::{HtmlValidate, Plan, rules};
use crate::fake::{FakeHost, fixture};
use crate::settings::{Engine, Settings};

fn every() -> Vec<String> {
    rules::all().iter().map(|rule| rule.id.clone()).collect()
}

fn context(view: Option<u32>, rules: Vec<String>) -> CheckContext {
    CheckContext {
        view,
        url: None,
        rules,
        args: String::new(),
    }
}

fn host() -> FakeHost {
    FakeHost::with_views(
        &[(1, "mobile"), (2, "desktop")],
        &fixture("invalid.html"),
        &fixture("answer-invalid.json"),
    )
}

#[test]
fn a_check_run_of_one_view_answers_its_findings() {
    let mut host = host();
    let found = HtmlValidate
        .run(&mut host, &context(Some(2), every()))
        .expect("findings");
    let rules: Vec<&str> = found.iter().map(|f| f.rule.as_str()).collect();
    assert_eq!(
        rules,
        ["no-dup-id", "close-order", "element-permitted-content"]
    );
    assert!(found.iter().all(|f| f.view == Some(2)));
    assert!(found.iter().all(|f| f.severity == Severity::Error));
    assert_eq!(found[0].path.as_deref(), Some("source:10:8"));
}

#[test]
fn a_check_run_without_a_view_covers_every_view() {
    let mut host = host();
    let found = HtmlValidate
        .run(&mut host, &context(None, every()))
        .expect("findings");
    assert_eq!(found.len(), 6);
    assert_eq!(
        host.evaluated.len(),
        1,
        "one validation for the shared source"
    );
}

#[test]
fn the_rules_left_on_decide_what_runs() {
    let mut host = host();
    let found = HtmlValidate
        .run(&mut host, &context(Some(1), vec!["no-dup-id".to_owned()]))
        .expect("findings");
    assert_eq!(found.len(), 1);
    let script = &host.evaluated[0].1;
    assert!(script.contains(r#""rules":{"no-dup-id":"error"}"#));
}

#[test]
fn nothing_left_on_runs_nothing() {
    let mut host = host();
    let found = HtmlValidate
        .run(&mut host, &context(None, Vec::new()))
        .expect("nothing");
    assert!(found.is_empty());
    assert!(host.cdp.is_empty() && host.evaluated.is_empty());
}

#[test]
fn a_failed_view_of_a_run_over_all_is_logged_and_the_rest_counts() {
    let mut host = host();
    host.sources.remove(&1);
    let found = HtmlValidate
        .run(&mut host, &context(None, every()))
        .expect("the other view");
    assert_eq!(found.len(), 3);
    assert_eq!(host.logs.len(), 1);
}

#[test]
fn the_failure_of_the_only_view_is_the_runs() {
    let mut host = host();
    host.sources.remove(&1);
    let failure = HtmlValidate
        .run(&mut host, &context(Some(1), every()))
        .expect_err("webkit");
    assert_eq!(failure.unserved_engine_in(1), Some("webkit"));
}

#[test]
fn the_nu_plan_needs_an_address() {
    let failure =
        Plan::new(Settings::default(), Some(Engine::Nu), every()).expect_err("no address");
    assert_eq!(
        failure.code(),
        Some("plugin.html-validate.errors.nu_address_missing")
    );
    let settings = Settings {
        nu_address: Some("http://localhost:8888".to_owned()),
        ..Settings::default()
    };
    let plan = Plan::new(settings, Some(Engine::Nu), every()).expect("plan");
    assert_eq!(plan.engine, Engine::Nu);
    assert!(!plan.is_empty());
}

#[test]
fn a_nu_plan_with_every_nu_rule_off_is_empty() {
    let settings = Settings {
        engine: Engine::Nu,
        nu_address: Some("http://localhost:8888".to_owned()),
        disabled_rules: vec!["nu-error".to_owned(), "nu-warning".to_owned()],
        ..Settings::default()
    };
    let plan = Plan::new(settings, None, every()).expect("plan");
    assert!(plan.is_empty(), "nu-info is below the lowest severity");
}

#[test]
fn describes_the_rule_table() {
    assert_eq!(HtmlValidate.describe().len(), rules::all().len());
}

#[test]
fn an_unknown_command_or_tool_is_refused() {
    let mut host = host();
    for failure in [
        HtmlValidate.run_command(&mut host, "other", ""),
        HtmlValidate.call_tool(&mut host, "other", ""),
    ] {
        assert_eq!(
            failure.expect_err("unknown").code(),
            Some(rb_plugin_api::codes::UNKNOWN_CONTRIBUTION)
        );
    }
}

#[test]
fn the_command_and_the_tool_run_the_validation() {
    let mut host = host();
    let text = HtmlValidate
        .call_tool(&mut host, "validate", "")
        .expect("tool");
    assert!(text.contains("no-dup-id"));
    HtmlValidate
        .run_command(&mut host, "validate", "")
        .expect("command");
    assert_eq!(host.reported.len(), 6);
}

// RB-094-06: the findings panel's run with the rendered DOM and a project's rule file.

#[test]
fn a_check_run_validates_the_rendered_dom_when_the_setting_says_so() {
    let mut host = host().set("source", "\"rendered\"");
    for view in [1, 2] {
        host.rendered.insert(
            view,
            Ok(serde_json::json!({ "html": "<html><body></body></html>" }).to_string()),
        );
    }
    let found = HtmlValidate
        .run(&mut host, &context(Some(1), every()))
        .expect("findings");
    assert!(found.iter().all(|f| f.selector.is_some()));
    assert_eq!(
        found[0].path.as_deref(),
        Some("html > body > main > p:nth-child(2)")
    );
    assert!(
        host.cdp
            .iter()
            .all(|(_, method, _)| method != "Page.getResourceContent")
    );
}

#[test]
fn a_trusted_project_switches_on_the_opt_in_rule() {
    let mut host = host();
    host.project = Some(Ok(
        br#"{"rules": {"no-unknown-attributes": "error"}}"#.to_vec()
    ));
    host.trusted = true;
    HtmlValidate
        .run(&mut host, &context(Some(1), every()))
        .expect("findings");
    let call = |host: &FakeHost| {
        host.evaluated[0]
            .1
            .replace(crate::script::RUNNER_SOURCE, "")
    };
    assert!(call(&host).contains(r#""no-unknown-attributes":"error""#));
    let mut host = self::host();
    HtmlValidate
        .run(&mut host, &context(Some(1), every()))
        .expect("findings");
    assert!(!call(&host).contains("no-unknown-attributes"));
}
