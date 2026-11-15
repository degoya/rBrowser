use serde_json::json;

use super::{
    CheckTarget, FailOn, FindingMessage, FindingsSummary, MergedFinding, fingerprint,
    normalize_path, normalize_url,
};
use crate::Severity;

fn merged(id: &str, plugins: &[&str], severity: Severity) -> MergedFinding {
    MergedFinding {
        id: id.to_owned(),
        rule_id: format!("{}.rule", plugins[0]),
        plugins: plugins.iter().map(|plugin| (*plugin).to_owned()).collect(),
        severity,
        message: FindingMessage {
            key: "plugin.seo.findings.alt".to_owned(),
            params: Default::default(),
        },
        views: vec![1],
        breakpoints: vec!["mobile".to_owned()],
        url: None,
        element: None,
        help_url: None,
        data: None,
        shared_id: None,
    }
}

#[test]
fn the_fingerprint_is_pinned_across_runs_and_platforms() {
    assert_eq!(
        fingerprint(
            "seo.images",
            Some("https://site.test/#top"),
            Some("html > body >  img")
        ),
        "4ef7e8f2df278863"
    );
    assert_eq!(
        fingerprint(
            "seo.images",
            Some(" https://site.test/ "),
            Some("html>body>img")
        ),
        "4ef7e8f2df278863",
        "the same element at the same page is the same finding"
    );
}

#[test]
fn the_fingerprint_tells_rules_pages_and_elements_apart() {
    let base = fingerprint(
        "seo.images",
        Some("https://site.test/"),
        Some("html>body>img"),
    );
    assert_ne!(
        base,
        fingerprint(
            "seo.links",
            Some("https://site.test/"),
            Some("html>body>img")
        )
    );
    assert_ne!(
        base,
        fingerprint(
            "seo.images",
            Some("https://site.test/a"),
            Some("html>body>img")
        )
    );
    assert_ne!(
        base,
        fingerprint(
            "seo.images",
            Some("https://site.test/"),
            Some("html>body>p>img")
        )
    );
    assert_ne!(
        base,
        fingerprint("seo.images", Some("https://site.test/"), None)
    );
    assert_eq!(base.len(), 16);
    assert!(base.chars().all(|c| c.is_ascii_hexdigit()));
}

#[test]
fn a_path_is_normalised_around_its_combinators_and_whitespace() {
    assert_eq!(
        normalize_path("  html >  body\tmain  +  p ~ a  "),
        "html>body main+p~a"
    );
    assert_eq!(normalize_path("div.card  span"), "div.card span");
    assert_eq!(normalize_path(""), "");
}

#[test]
fn a_url_loses_its_fragment_and_outer_whitespace() {
    assert_eq!(
        normalize_url(" https://site.test/a?b=1#c "),
        "https://site.test/a?b=1"
    );
    assert_eq!(normalize_url("https://site.test/"), "https://site.test/");
}

#[test]
fn the_target_is_all_or_a_view_id_in_json() {
    assert_eq!(json!(CheckTarget::All), json!("all"));
    assert_eq!(json!(CheckTarget::View(3)), json!(3));
    let all: CheckTarget = serde_json::from_value(json!("all")).expect("all");
    assert_eq!(all, CheckTarget::All);
    let view: CheckTarget = serde_json::from_value(json!(4)).expect("a view");
    assert_eq!(view, CheckTarget::View(4));
    assert!(serde_json::from_value::<CheckTarget>(json!("some")).is_err());
}

#[test]
fn the_summary_counts_per_severity_and_plugin() {
    let findings = vec![
        merged("a", &["seo"], Severity::Error),
        merged("b", &["seo", "a11y-axe"], Severity::Warning),
        merged("c", &["console"], Severity::Info),
        merged("d", &["console"], Severity::Error),
    ];
    let summary = FindingsSummary::of(&findings);
    assert_eq!(summary.total, 4);
    assert_eq!((summary.error, summary.warning, summary.info), (2, 1, 1));
    assert_eq!(summary.by_plugin.get("seo"), Some(&2));
    assert_eq!(summary.by_plugin.get("a11y-axe"), Some(&1));
    assert_eq!(summary.by_plugin.get("console"), Some(&2));
}

#[test]
fn fail_on_holds_the_threshold() {
    let warning_only = FindingsSummary::of(&[merged("a", &["seo"], Severity::Warning)]);
    assert!(!FailOn::Error.fails(&warning_only));
    assert!(FailOn::Warning.fails(&warning_only));
    let with_error = FindingsSummary::of(&[merged("a", &["seo"], Severity::Error)]);
    assert!(FailOn::Error.fails(&with_error));
    assert!(!FailOn::Error.fails(&FindingsSummary::default()));
    assert_eq!(
        serde_json::from_value::<FailOn>(json!("warning")).expect("parses"),
        FailOn::Warning
    );
}

#[test]
fn a_merged_finding_is_camel_case_json() {
    let mut finding = merged("a", &["seo"], Severity::Error);
    finding.shared_id = Some("img-alt-missing".to_owned());
    finding.help_url = Some("https://help.test/alt".to_owned());
    let value = json!(finding);
    assert_eq!(value["ruleId"], "seo.rule");
    assert_eq!(value["sharedId"], "img-alt-missing");
    assert_eq!(value["helpUrl"], "https://help.test/alt");
    assert_eq!(value["severity"], "error");
    assert_eq!(value["message"]["key"], "plugin.seo.findings.alt");
    let back: MergedFinding = serde_json::from_value(value).expect("round trip");
    assert_eq!(back, finding);
}
