use rb_plugin_types::Severity;
use serde_json::json;

use super::{ChecksPolicy, ChecksSettings, DEFAULT_KEEP_RUNS, DEFAULT_PARALLEL, RuleLevel};

fn policy(value: serde_json::Value) -> ChecksPolicy {
    ChecksPolicy::new(&ChecksSettings::from_value(Some(&value)))
}

#[test]
fn the_settings_default_and_clamp() {
    let defaults = ChecksSettings::from_value(None);
    assert_eq!(defaults.parallel(), DEFAULT_PARALLEL);
    assert_eq!(defaults.keep_runs(), DEFAULT_KEEP_RUNS);
    let wild = ChecksSettings::from_value(Some(&json!({ "parallel": 0, "keepRuns": 100_000 })));
    assert_eq!(wild.parallel(), 1);
    assert_eq!(wild.keep_runs(), 100);
    let broken = ChecksSettings::from_value(Some(&json!({ "parallel": "many" })));
    assert_eq!(
        broken.parallel(),
        DEFAULT_PARALLEL,
        "a broken section is the default"
    );
}

#[test]
fn a_rule_switched_off_is_dropped_and_error_raises_it() {
    let policy = policy(json!({
        "rules": { "seo.images": "off", "seo.title": "error", "seo.meta": "info" }
    }));
    assert_eq!(policy.severity("seo.images", Severity::Error), None);
    assert!(!policy.enabled("seo.images"));
    assert_eq!(
        policy.severity("seo.title", Severity::Warning),
        Some(Severity::Error)
    );
    assert_eq!(
        policy.severity("seo.meta", Severity::Error),
        Some(Severity::Info)
    );
    assert_eq!(
        policy.severity("seo.other", Severity::Warning),
        Some(Severity::Warning),
        "without an override the reported severity holds"
    );
    assert_eq!(policy.level("seo.title"), Some(RuleLevel::Error));
}

#[test]
fn an_unknown_level_is_no_override() {
    let policy = policy(json!({ "rules": { "seo.images": "loud" } }));
    assert_eq!(
        policy.severity("seo.images", Severity::Warning),
        Some(Severity::Warning)
    );
}

#[test]
fn ignore_patterns_match_every_field_they_name() {
    let policy = policy(json!({
        "ignore": [
            { "rule": "a11y-axe.*" },
            { "url": "https://site.test/admin/*" },
            { "rule": "seo.images", "selector": "img.ad*" },
            {}
        ]
    }));
    assert!(policy.ignored("a11y-axe.image-alt", None, None));
    assert!(policy.ignored("seo.title", Some("https://site.test/admin/users"), None));
    assert!(!policy.ignored("seo.title", Some("https://site.test/"), None));
    assert!(policy.ignored(
        "seo.images",
        Some("https://site.test/"),
        Some("img.ad-banner")
    ));
    assert!(
        !policy.ignored("seo.images", Some("https://site.test/"), Some("img.hero")),
        "every field named has to match"
    );
    assert!(
        !policy.ignored("seo.images", Some("https://site.test/"), None),
        "a finding without a selector does not match a selector pattern"
    );
    assert!(
        !policy.ignored("seo.links", None, None),
        "an empty pattern ignores nothing"
    );
}

#[test]
fn a_broken_glob_is_left_out() {
    let policy = policy(json!({ "ignore": [{ "rule": "seo.[" }, { "rule": "seo.title" }] }));
    assert!(policy.ignored("seo.title", None, None));
    assert!(!policy.ignored("seo.[", None, None));
}
