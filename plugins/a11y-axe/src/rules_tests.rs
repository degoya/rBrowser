use std::collections::{BTreeMap, BTreeSet};

use rb_plugin_api::Severity;
use rb_plugin_api::check::{MAX_RULES, valid_rule_id};

use super::*;

fn tags(list: &[&str]) -> Vec<String> {
    list.iter().map(|tag| (*tag).to_owned()).collect()
}

fn every() -> Vec<String> {
    all().iter().map(|rule| rule.id.clone()).collect()
}

/// A snapshot of `tests/fixtures/<name>.txt`: one rule id per line.
fn snapshot(name: &str) -> Vec<String> {
    let path = format!("{}/tests/fixtures/{name}.txt", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(path)
        .expect("the snapshot exists")
        .lines()
        .map(str::to_owned)
        .collect()
}

#[test]
fn the_table_is_the_pinned_axe_core_with_valid_ids() {
    assert_eq!(axe_version(), "4.14.0");
    assert_eq!(all().len(), 105);
    assert!(all().len() <= MAX_RULES);
    for rule in all() {
        assert!(valid_rule_id(&rule.id), "{}", rule.id);
    }
}

#[test]
fn every_rule_is_described_with_its_title_key_severity_and_help_page() {
    let described = describe();
    assert_eq!(described.len(), all().len());
    let image_alt = described
        .iter()
        .find(|rule| rule.id == "image-alt")
        .expect("image-alt");
    assert_eq!(image_alt.title, "plugin.a11y-axe.rules.image-alt");
    assert_eq!(image_alt.severity, Severity::Error);
    assert_eq!(image_alt.shared_id.as_deref(), Some("img-alt-missing"));
    assert!(
        image_alt
            .help_url
            .as_deref()
            .is_some_and(|url| url.starts_with("https://dequeuniversity.com/rules/axe/4.14/"))
    );
}

#[test]
fn the_shared_ids_name_rules_of_the_table_and_are_valid_ids() {
    for (rule, shared) in SHARED_IDS {
        assert!(all().iter().any(|entry| entry.id == rule), "{rule}");
        assert!(valid_rule_id(shared), "{shared}");
    }
    assert_eq!(shared_id("link-name"), Some("link-name-missing"));
    assert_eq!(shared_id("html-has-lang"), Some("html-lang-missing"));
    assert_eq!(shared_id("label"), Some("form-label-missing"));
    assert_eq!(shared_id("page-has-heading-one"), Some("h1-missing"));
    assert_eq!(shared_id("region"), Some("content-outside-landmarks"));
    assert_eq!(shared_id("button-name"), None);
}

#[test]
fn an_impact_maps_to_a_severity() {
    assert_eq!(severity_of(Some("critical")), Severity::Error);
    assert_eq!(severity_of(Some("serious")), Severity::Error);
    assert_eq!(severity_of(Some("moderate")), Severity::Warning);
    assert_eq!(severity_of(Some("minor")), Severity::Info);
    assert_eq!(severity_of(None), Severity::Info);
}

#[test]
fn a_wcag_level_includes_the_lower_levels_and_versions() {
    let expanded = expand_tags(&tags(&["wcag22aa"]));
    let expected: BTreeSet<String> = tags(&[
        "wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22a", "wcag22aa",
    ])
    .into_iter()
    .collect();
    assert_eq!(expanded, expected);
    assert_eq!(
        expand_tags(&tags(&["wcag2a"])),
        tags(&["wcag2a"]).into_iter().collect()
    );
    assert!(expand_tags(&tags(&["wcag21aaa"])).contains("wcag2aaa"));
    assert_eq!(
        expand_tags(&tags(&["best-practice", "wcag2"])),
        tags(&["best-practice", "wcag2"]).into_iter().collect()
    );
}

#[test]
fn the_default_level_runs_wcag_rules_only() {
    let selected = select(&tags(&["wcag22aa"]), &BTreeMap::new(), &every());
    assert_eq!(selected, snapshot("rules-wcag22aa"));
    for id in ["image-alt", "button-name", "color-contrast", "target-size"] {
        assert!(selected.iter().any(|rule| rule == id), "{id}");
    }
    // best practice, AAA, experimental and deprecated rules stay out.
    for id in [
        "region",
        "color-contrast-enhanced",
        "p-as-heading",
        "duplicate-id",
    ] {
        assert!(!selected.iter().any(|rule| rule == id), "{id}");
    }
}

#[test]
fn wcag2a_runs_fewer_rules_than_wcag22aa() {
    let narrow = select(&tags(&["wcag2a"]), &BTreeMap::new(), &every());
    let wide = select(&tags(&["wcag22aa"]), &BTreeMap::new(), &every());
    assert_eq!(narrow, snapshot("rules-wcag2a"));
    assert!(narrow.len() < wide.len());
    assert!(narrow.iter().all(|rule| wide.contains(rule)));
    assert!(!narrow.iter().any(|rule| rule == "color-contrast"));
}

#[test]
fn switches_add_and_remove_single_rules() {
    let switches = BTreeMap::from([
        ("region".to_owned(), true),
        ("image-alt".to_owned(), false),
        ("p-as-heading".to_owned(), true),
    ]);
    let selected = select(&tags(&["wcag2a"]), &switches, &every());
    assert!(selected.iter().any(|rule| rule == "region"));
    assert!(selected.iter().any(|rule| rule == "p-as-heading"));
    assert!(!selected.iter().any(|rule| rule == "image-alt"));
}

#[test]
fn an_opt_in_tag_runs_its_rules_when_asked_for() {
    let selected = select(&tags(&["experimental"]), &BTreeMap::new(), &every());
    assert!(selected.iter().any(|rule| rule == "p-as-heading"));
}

#[test]
fn only_the_rules_the_person_left_on_run() {
    let enabled = tags(&["image-alt", "region"]);
    let selected = select(&tags(&["wcag22aa"]), &BTreeMap::new(), &enabled);
    assert_eq!(selected, tags(&["image-alt"]));
}
