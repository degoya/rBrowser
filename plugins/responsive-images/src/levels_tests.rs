use rb_plugin_api::{Finding, RULE_LEVELS_SETTING, Severity};
use serde_json::json;

use super::*;
use crate::fake::FakeHost;

fn finding(rule: &str, severity: Severity) -> Finding {
    Finding {
        rule: rule.to_owned(),
        severity,
        message: format!("plugin.responsive-images.rules.{rule}"),
        ..Finding::default()
    }
}

#[test]
fn a_level_sets_the_severity_and_off_drops_the_finding() {
    let levels = RuleLevels::from_value(&json!({
        "oversized": "error",
        "lcp": "off",
        "undersized": "loud",
        "sizes-missing": 3
    }));
    let raised = levels
        .apply(finding("oversized", Severity::Warning))
        .expect("kept");
    assert_eq!(raised.severity, Severity::Error);
    assert_eq!(levels.apply(finding("lcp", Severity::Error)), None);
    for rule in ["undersized", "sizes-missing", "other"] {
        let kept = levels.apply(finding(rule, Severity::Info)).expect(rule);
        assert_eq!(kept.severity, Severity::Info, "{rule} keeps its own");
    }
}

#[test]
fn the_levels_come_from_the_host_and_nothing_is_no_override() {
    let mut host = FakeHost::default();
    assert!(RuleLevels::read(&mut host).is_empty());
    host.settings
        .insert(RULE_LEVELS_SETTING.to_owned(), "not json".to_owned());
    assert!(RuleLevels::read(&mut host).is_empty());
    host.settings.insert(
        RULE_LEVELS_SETTING.to_owned(),
        r#"{ "oversized": "info" }"#.to_owned(),
    );
    let levels = RuleLevels::read(&mut host);
    assert_eq!(
        levels
            .apply(finding("oversized", Severity::Warning))
            .map(|kept| kept.severity),
        Some(Severity::Info)
    );
}
