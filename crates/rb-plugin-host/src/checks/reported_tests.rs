use rb_plugin_types::{CheckTarget, Finding, Severity};
use serde_json::json;

use super::reported_run;
use crate::checks::{ChecksSettings, RunRequest, RunView};

fn finding(rule: &str, view: Option<u32>, selector: &str) -> Finding {
    Finding {
        rule: rule.to_owned(),
        severity: Severity::Warning,
        message: "plugin.console.found".to_owned(),
        view,
        selector: Some(selector.to_owned()),
        ..Finding::default()
    }
}

fn request() -> RunRequest {
    RunRequest {
        run_id: "run-0000000000002".to_owned(),
        started_at: 2,
        target: CheckTarget::All,
        views: vec![
            RunView {
                id: 1,
                breakpoint: Some("mobile".to_owned()),
                url: Some("https://site.test/".to_owned()),
            },
            RunView {
                id: 2,
                breakpoint: Some("desktop".to_owned()),
                url: Some("https://site.test/".to_owned()),
            },
        ],
        url: None,
        plugins: None,
        args: "{}".to_owned(),
    }
}

#[test]
fn reported_findings_become_one_run_of_the_plugin_merged_across_views() {
    let settings = ChecksSettings::default();
    let file = reported_run(
        &settings,
        "console",
        vec![
            finding("error", Some(1), "#a"),
            finding("error", Some(2), "#a"),
        ],
        request(),
    );
    assert_eq!(file.plugins, vec!["console".to_owned()]);
    assert_eq!(file.findings.len(), 1);
    let only = &file.findings[0];
    assert_eq!(only.rule_id, "console.error");
    assert_eq!(only.views, vec![1, 2]);
    assert_eq!(
        only.breakpoints,
        vec!["mobile".to_owned(), "desktop".to_owned()]
    );
    assert_eq!(only.url.as_deref(), Some("https://site.test/"));
    assert_eq!(file.summary.warning, 1);
}

#[test]
fn the_persons_rule_overrides_and_ignores_hold_for_reported_findings() {
    let settings = ChecksSettings::from_value(Some(&json!({
        "rules": { "console.error": "error", "console.warning": "off" },
        "ignore": [{ "rule": "console.error", "selector": "#ignored" }],
    })));
    let file = reported_run(
        &settings,
        "console",
        vec![
            finding("error", Some(1), "#a"),
            finding("warning", Some(1), "#b"),
            finding("error", Some(1), "#ignored"),
        ],
        request(),
    );
    assert_eq!(file.findings.len(), 1);
    assert_eq!(file.findings[0].severity, Severity::Error);
}
