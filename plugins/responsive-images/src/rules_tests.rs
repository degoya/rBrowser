use std::collections::BTreeMap;

use rb_plugin_api::Severity;

use super::*;
use crate::page::{Image, Issue, PageImages};

fn issue(rule: &str, variant: Option<&str>, params: &[(&str, &str)]) -> Issue {
    Issue {
        rule: rule.to_owned(),
        variant: variant.map(str::to_owned),
        params: params
            .iter()
            .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
            .collect::<BTreeMap<_, _>>(),
    }
}

fn page(issues: Vec<Issue>) -> PageImages {
    PageImages {
        url: "https://example.test/".to_owned(),
        dpr: 2.0,
        images: vec![Image {
            selector: "#hero".to_owned(),
            path: "1,HTML,1,BODY,0,IMG".to_owned(),
            current_src: "https://example.test/hero-1600.jpg".to_owned(),
            issues,
            ..Image::default()
        }],
        ..PageImages::default()
    }
}

#[test]
fn every_rule_is_described_with_its_title_help_and_shared_id() {
    let described = describe();
    let ids: Vec<&str> = described.iter().map(|rule| rule.id.as_str()).collect();
    assert_eq!(ids, [OVERSIZED, UNDERSIZED, SIZES_MISSING, LCP]);
    for rule in &described {
        assert_eq!(
            rule.title,
            format!("plugin.responsive-images.rules.{}", rule.id)
        );
        assert!(
            rule.help_url
                .as_deref()
                .is_some_and(|url| url.starts_with("https://"))
        );
    }
    let shared: Vec<Option<&str>> = described
        .iter()
        .map(|rule| rule.shared_id.as_deref())
        .collect();
    assert_eq!(
        shared,
        [
            Some("image-oversized"),
            Some("image-undersized"),
            None,
            Some("lcp-image-discovery")
        ]
    );
    assert_eq!(severity_of("nothing"), Severity::Info);
}

#[test]
fn an_issue_keeps_only_known_rules_variants_and_parameters() {
    assert_eq!(known_issue(issue("made-up", None, &[])), None);
    assert_eq!(known_issue(issue(OVERSIZED, Some("img"), &[])), None);
    assert_eq!(known_issue(issue(LCP, None, &[])), None);
    assert_eq!(
        known_issue(issue(SIZES_MISSING, Some("picture"), &[])),
        None
    );
    let kept = known_issue(issue(
        OVERSIZED,
        None,
        &[("dpr", "2"), ("ratio", "2"), ("onclick", "x")],
    ))
    .expect("known");
    assert_eq!(kept.params.keys().collect::<Vec<_>>(), ["dpr", "ratio"]);
    assert!(known_issue(issue(LCP, Some("priority"), &[])).is_some());
}

#[test]
fn a_finding_names_the_view_density_image_and_element() {
    let page = page(vec![
        issue(OVERSIZED, None, &[("dpr", "2"), ("ratio", "2")]),
        issue(LCP, Some("lazy"), &[]),
    ]);
    let found = findings(7, &page, &[]);
    assert_eq!(found.len(), 2);
    let first = &found[0];
    assert_eq!(first.rule, OVERSIZED);
    assert_eq!(first.severity, Severity::Warning);
    assert_eq!(first.message, "plugin.responsive-images.findings.oversized");
    assert_eq!(first.view, Some(7));
    assert_eq!(first.url.as_deref(), Some("https://example.test/"));
    assert_eq!(first.selector.as_deref(), Some("#hero"));
    assert_eq!(first.path.as_deref(), Some("1,HTML,1,BODY,0,IMG"));
    assert_eq!(
        first.data.as_deref(),
        Some(r#"{"panel":"responsive-images"}"#)
    );
    assert!(first.params.contains(&(
        "image".to_owned(),
        "https://example.test/hero-1600.jpg".to_owned()
    )));
    // The LCP issue carried no density: the page's is added.
    let second = &found[1];
    assert_eq!(second.message, "plugin.responsive-images.findings.lcp.lazy");
    assert!(second.params.contains(&("dpr".to_owned(), "2".to_owned())));
}

#[test]
fn only_the_enabled_rules_become_findings() {
    let page = page(vec![
        issue(OVERSIZED, None, &[]),
        issue(SIZES_MISSING, Some("img"), &[]),
    ]);
    let found = findings(1, &page, &[SIZES_MISSING.to_owned()]);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].rule, SIZES_MISSING);
    assert_eq!(
        found[0].message,
        "plugin.responsive-images.findings.sizes-missing.img"
    );
}
