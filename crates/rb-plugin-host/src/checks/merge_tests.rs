use rb_plugin_types::{Severity, fingerprint};

use super::{Report, merge};
use crate::checks::testing::finding;

fn report(plugin: &str, rule: &str, view: u32, selector: &str) -> Report {
    let mut found = finding(rule, Severity::Warning, selector);
    found.url = Some("https://site.test/".to_owned());
    Report {
        plugin: plugin.to_owned(),
        rule_id: format!("{plugin}.{rule}"),
        shared_id: None,
        help_url: None,
        severity: Severity::Warning,
        view: Some(view),
        breakpoint: Some(["", "mobile", "tablet", "desktop"][view as usize].to_owned()),
        finding: found,
    }
}

#[test]
fn the_same_finding_in_several_views_is_one_with_their_views() {
    let merged = merge(vec![
        report("seo", "images", 3, "img"),
        report("seo", "images", 1, "img"),
        report("seo", "images", 1, "img"),
    ]);
    assert_eq!(merged.len(), 1);
    let only = &merged[0];
    assert_eq!(only.views, vec![1, 3]);
    assert_eq!(only.breakpoints, vec!["mobile", "desktop"]);
    assert_eq!(only.plugins, vec!["seo"]);
    assert_eq!(
        only.id,
        fingerprint(
            "seo.images",
            Some("https://site.test/"),
            Some("html>body>img")
        )
    );
    assert_eq!(
        only.message.params.get("count").map(String::as_str),
        Some("2")
    );
    assert_eq!(
        only.element
            .as_ref()
            .and_then(|element| element.path.as_deref()),
        Some("html>body>img")
    );
}

#[test]
fn two_plugins_with_one_shared_id_at_one_element_are_one_finding() {
    let mut seo = report("seo", "images", 1, "img.hero");
    seo.shared_id = Some("img-alt-missing".to_owned());
    let mut axe = report("a11y-axe", "image-alt", 2, "img.hero");
    axe.shared_id = Some("img-alt-missing".to_owned());
    axe.severity = Severity::Error;
    let mut elsewhere = report("a11y-axe", "image-alt", 2, "img.logo");
    elsewhere.shared_id = Some("img-alt-missing".to_owned());

    let merged = merge(vec![seo, axe, elsewhere]);
    assert_eq!(merged.len(), 2, "another element stays its own finding");
    let shared = merged
        .iter()
        .find(|finding| finding.plugins.len() == 2)
        .expect("one finding of both plugins");
    assert_eq!(shared.plugins, vec!["seo", "a11y-axe"]);
    assert_eq!(shared.rule_id, "seo.images", "the first report's rule");
    assert_eq!(shared.severity, Severity::Error, "the highest severity");
    assert_eq!(shared.views, vec![1, 2]);
    assert_eq!(shared.shared_id.as_deref(), Some("img-alt-missing"));
}

#[test]
fn different_rules_at_one_element_stay_apart() {
    let merged = merge(vec![
        report("seo", "images", 1, "img"),
        report("seo", "lazy", 1, "img"),
    ]);
    assert_eq!(merged.len(), 2);
}

#[test]
fn the_order_is_by_severity_then_rule() {
    let mut error = report("seo", "zeta", 1, "a");
    error.severity = Severity::Error;
    let merged = merge(vec![
        report("seo", "beta", 1, "a"),
        report("seo", "alpha", 1, "a"),
        error,
    ]);
    let rules: Vec<&str> = merged
        .iter()
        .map(|finding| finding.rule_id.as_str())
        .collect();
    assert_eq!(rules, vec!["seo.zeta", "seo.alpha", "seo.beta"]);
}

#[test]
fn data_is_kept_as_json() {
    let mut with_data = report("seo", "images", 1, "img");
    with_data.finding.data = Some("{\"width\":40}".to_owned());
    let merged = merge(vec![with_data]);
    assert_eq!(merged[0].data, Some(serde_json::json!({ "width": 40 })));
}
