use rb_plugin_api::check::valid_rule_id;
use rb_plugin_api::{Severity, ViewInfo};
use serde_json::{Value, json};

use super::*;
use crate::fake::overflow;
use crate::plan::{Issue, Range};

fn view() -> ViewInfo {
    ViewInfo {
        id: 3,
        label: "Tablet".to_owned(),
        breakpoint: "tablet".to_owned(),
        engine: "chromium".to_owned(),
        width: 768,
        height: 1024,
        dpr: 2.0,
        url: "https://example.test/".to_owned(),
        title: String::new(),
    }
}

fn param<'a>(finding: &'a Finding, name: &str) -> Option<&'a str> {
    finding
        .params
        .iter()
        .find(|(key, _)| key == name)
        .map(|(_, value)| value.as_str())
}

#[test]
fn every_kind_is_a_rule_with_its_own_title() {
    let rules = describe();
    let ids: Vec<&str> = rules.iter().map(|rule| rule.id.as_str()).collect();
    assert_eq!(ids, crate::settings::KINDS);
    for rule in &rules {
        assert!(valid_rule_id(&rule.id), "{}", rule.id);
        assert_eq!(rule.title, format!("plugin.width-sweep.rules.{}", rule.id));
    }
    assert_eq!(
        rules[0].severity,
        Severity::Error,
        "an overflow scrolls the page"
    );
}

#[test]
fn a_range_is_one_finding_naming_its_widths_and_element() {
    let found = findings(
        &[Range {
            issue: overflow(".hero img"),
            from: 700,
            to: 740,
        }],
        &view(),
    );
    assert_eq!(found.len(), 1);
    let finding = &found[0];
    assert_eq!(finding.rule, "overflow");
    assert_eq!(finding.message, "plugin.width-sweep.findings.overflow");
    assert_eq!(param(finding, "from"), Some("700"));
    assert_eq!(param(finding, "to"), Some("740"));
    assert_eq!(param(finding, "selector"), Some(".hero img"));
    assert_eq!(finding.view, Some(3));
    assert_eq!(finding.url.as_deref(), Some("https://example.test/"));
    assert_eq!(finding.selector.as_deref(), Some(".hero img"));
    assert_eq!(finding.path.as_deref(), Some("html > body > .hero img"));
    let data: Value = serde_json::from_str(finding.data.as_deref().expect("data")).expect("JSON");
    assert_eq!(
        data,
        json!({ "kind": "overflow", "from": 700, "to": 740, "width": 720 })
    );
}

#[test]
fn an_overlap_names_both_elements_and_the_findings_run_by_width() {
    let overlap = Issue {
        kind: "overlap".to_owned(),
        selector: "nav > a:nth-of-type(2)".to_owned(),
        path: String::new(),
        other: Some("nav > a:nth-of-type(1)".to_owned()),
    };
    let found = findings(
        &[
            Range {
                issue: overlap,
                from: 900,
                to: 910,
            },
            Range {
                issue: overflow("table"),
                from: 320,
                to: 359,
            },
        ],
        &view(),
    );
    assert_eq!(found[0].rule, "overflow", "the narrower range first");
    let overlap = &found[1];
    assert_eq!(overlap.severity, Severity::Warning);
    assert_eq!(param(overlap, "other"), Some("nav > a:nth-of-type(1)"));
    assert_eq!(overlap.path, None, "no path: the selector alone");
}
