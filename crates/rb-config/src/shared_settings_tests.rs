use serde_json::json;

use super::*;
use crate::{ProjectConfig, UserSettings};

#[test]
fn the_findings_keys_live_in_the_checks_section_beside_the_plugins() {
    let config: ProjectConfig = serde_json::from_value(json!({
        "version": 1,
        "checks": {
            "axe": { "tags": ["wcag22aa"] },
            "parallel": 2,
            "rules": { "seo.images": "off" },
            "ignore": [{ "rule": "seo.*", "url": "*/admin/*" }],
            "contrast": { "level": "aa" }
        }
    }))
    .expect("a config");
    let findings = &config.checks.findings;
    assert_eq!(findings.parallel, Some(2));
    assert_eq!(findings.keep_runs, None);
    assert_eq!(findings.rules.get("seo.images"), Some(&RuleSetting::Off));
    assert_eq!(findings.ignore[0].rule.as_deref(), Some("seo.*"));
    assert_eq!(config.checks.extra["axe"], json!({ "tags": ["wcag22aa"] }));
    assert!(config.checks.extra.contains_key("contrast"));
    assert!(!config.checks.extra.contains_key("parallel"));
}

#[test]
fn the_user_settings_read_the_same_section() {
    let user: UserSettings = serde_json::from_value(json!({
        "checks": { "keepRuns": 3, "rules": { "a11y.alt": "warning" } }
    }))
    .expect("user settings");
    assert_eq!(user.checks.keep_runs, Some(3));
    assert!(!user.extra.contains_key("checks"));
}

#[test]
fn the_project_wins_per_key_and_rules_merge() {
    let project = FindingsSettings {
        parallel: Some(8),
        rules: BTreeMap::from([("a.x".to_owned(), RuleSetting::Error)]),
        ..FindingsSettings::default()
    };
    let user = FindingsSettings {
        parallel: Some(2),
        keep_runs: Some(5),
        rules: BTreeMap::from([
            ("a.x".to_owned(), RuleSetting::Off),
            ("b.y".to_owned(), RuleSetting::Info),
        ]),
        ignore: vec![IgnoreEntry {
            rule: Some("c.*".to_owned()),
            ..IgnoreEntry::default()
        }],
        ..FindingsSettings::default()
    };
    let merged = project.over(&user);
    assert_eq!(merged.parallel, Some(8));
    assert_eq!(merged.keep_runs, Some(5));
    assert_eq!(merged.rules.get("a.x"), Some(&RuleSetting::Error));
    assert_eq!(merged.rules.get("b.y"), Some(&RuleSetting::Info));
    assert_eq!(merged.ignore, user.ignore);
}

#[test]
fn an_unknown_rule_level_or_ignore_field_is_refused() {
    let bad_level = json!({ "version": 1, "checks": { "rules": { "a.x": "fatal" } } });
    assert!(serde_json::from_value::<ProjectConfig>(bad_level).is_err());
    let bad_field = json!({ "version": 1, "checks": { "ignore": [{ "path": "x" }] } });
    assert!(serde_json::from_value::<ProjectConfig>(bad_field).is_err());
}

#[test]
fn the_capture_section_is_typed_in_both_files() {
    let config: ProjectConfig = serde_json::from_value(json!({
        "version": 1,
        "capture": { "pathPattern": "{env}/{breakpoint}", "elementPadding": 8 }
    }))
    .expect("a config");
    assert_eq!(
        config.capture.path_pattern.as_deref(),
        Some("{env}/{breakpoint}")
    );
    assert_eq!(config.capture.element_padding, Some(8));
    assert!(!config.extra.contains_key("capture"));
    let user: UserSettings =
        serde_json::from_value(json!({ "capture": { "dir": "C:\\shots" } })).expect("settings");
    assert_eq!(user.capture.dir.as_deref(), Some("C:\\shots"));
    assert!(!user.extra.contains_key("capture"));
    // Unset keys stay out, so the project wins only where it sets one.
    assert_eq!(
        serde_json::to_value(&config.capture).expect("serializes"),
        json!({ "pathPattern": "{env}/{breakpoint}", "elementPadding": 8 })
    );
}
