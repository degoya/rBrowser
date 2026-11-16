use std::collections::BTreeSet;

use rb_plugin_api::check::{MAX_RULES, valid_rule_id};

use super::*;
use crate::config::IgnorePattern;

#[test]
fn every_rule_id_is_valid_unique_and_grouped() {
    let mut seen = BTreeSet::new();
    assert!(RULES.len() <= MAX_RULES);
    for rule in RULES {
        assert!(valid_rule_id(rule.id), "{}", rule.id);
        assert!(seen.insert(rule.id), "{} twice", rule.id);
        assert!(rule.id.contains('.'), "{} names its group", rule.id);
        if let Some(shared) = rule.shared {
            assert!(valid_rule_id(shared));
        }
    }
}

#[test]
fn the_alt_and_heading_groups_share_their_ids_with_axe_and_the_outline() {
    let shared: Vec<_> = describe()
        .into_iter()
        .filter_map(|rule| Some((rule.id, rule.shared_id?)))
        .collect();
    assert_eq!(
        shared,
        [
            (
                "meta.lang_missing".to_owned(),
                "html-lang-missing".to_owned()
            ),
            ("headings.h1_missing".to_owned(), "h1-missing".to_owned()),
            ("headings.h1_multiple".to_owned(), "h1-multiple".to_owned()),
            ("headings.skipped".to_owned(), "heading-order".to_owned()),
            (
                "images.alt_missing".to_owned(),
                "img-alt-missing".to_owned()
            ),
            (
                "images.linked_no_name".to_owned(),
                "link-name-missing".to_owned()
            ),
        ]
    );
}

#[test]
fn the_sink_applies_the_run_the_levels_and_the_ignore_patterns() {
    let mut config = Config::default();
    config
        .rules
        .insert("meta".to_owned(), Level::Severity(Severity::Info));
    config
        .rules
        .insert("headings.skipped".to_owned(), Level::Off);
    config.ignore.push(IgnorePattern {
        selector: Some("#legacy".to_owned()),
        ..IgnorePattern::default()
    });
    let enabled = Vec::new();
    let mut sink = Sink::new(&config, &enabled);
    sink.page("https://a.test/", Some(3));
    sink.emit("meta.title_missing", "meta.title_missing", None, &[]);
    sink.emit("headings.skipped", "headings.skipped", None, &[]);
    sink.emit(
        "images.alt_missing",
        "images.alt_missing",
        Some(Element::at("#legacy")),
        &[],
    );
    sink.emit("no.such_rule", "x", None, &[]);
    assert_eq!(sink.findings.len(), 1);
    let finding = &sink.findings[0];
    assert_eq!(finding.severity, Severity::Info);
    assert_eq!(finding.message, "plugin.seo.findings.meta.title_missing");
    assert_eq!(finding.view, Some(3));
    assert_eq!(finding.url.as_deref(), Some("https://a.test/"));

    let only = vec!["headings.h1_missing".to_owned()];
    let sink = Sink::new(&config, &only);
    assert!(sink.wants("headings.h1_missing"));
    assert!(!sink.wants("meta.title_missing"));
}

#[test]
fn short_cuts_long_text() {
    assert_eq!(short("abc", 3), "abc");
    assert_eq!(short("abcdef", 3), "abc...");
}

#[test]
fn every_rule_belongs_to_a_group_a_run_may_ask_for() {
    for rule in RULES {
        let group = rule.id.split_once('.').map(|(group, _)| group);
        assert!(group.is_some_and(|g| GROUPS.contains(&g)), "{}", rule.id);
    }
    let wellknown = of_groups(&["wellknown".to_owned()]);
    assert_eq!(wellknown.len(), 10);
    assert!(wellknown.iter().all(|id| id.starts_with("wellknown.")));
    assert!(of_groups(&[]).is_empty());
}
