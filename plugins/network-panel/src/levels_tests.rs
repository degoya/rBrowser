use rb_plugin_api::{Finding, RULE_LEVELS_SETTING, Severity};
use serde_json::json;

use super::*;
use crate::fake_host::FakeHost;

fn finding(rule: &str, severity: Severity) -> Finding {
    Finding {
        rule: rule.to_owned(),
        severity,
        message: format!("plugin.network-panel.findings.{rule}"),
        ..Finding::default()
    }
}

#[test]
fn a_level_sets_the_severity_and_off_drops_the_finding() {
    let levels = RuleLevels::from_value(&json!({
        "endpoint.slow": "error",
        "endpoint.duplicate": "off",
        "endpoint.no-cache": "loud",
        "endpoint.uncompressed": 3
    }));
    let raised = levels
        .apply(finding("endpoint.slow", Severity::Warning))
        .expect("kept");
    assert_eq!(raised.severity, Severity::Error);
    assert_eq!(
        levels.apply(finding("endpoint.duplicate", Severity::Warning)),
        None
    );
    for rule in [
        "endpoint.no-cache",
        "endpoint.uncompressed",
        "endpoint.n-plus-one",
    ] {
        let kept = levels.apply(finding(rule, Severity::Info)).expect(rule);
        assert_eq!(kept.severity, Severity::Info, "{rule} keeps its own");
    }
}

#[test]
fn the_levels_come_from_the_host_and_nothing_is_no_override() {
    let mut host = FakeHost::default();
    assert!(RuleLevels::read(&mut host).is_empty());
    host.set(RULE_LEVELS_SETTING, "not json");
    assert!(RuleLevels::read(&mut host).is_empty());
    host.set(RULE_LEVELS_SETTING, r#"{ "endpoint.slow": "info" }"#);
    let levels = RuleLevels::read(&mut host);
    assert_eq!(
        levels
            .apply(finding("endpoint.slow", Severity::Warning))
            .map(|kept| kept.severity),
        Some(Severity::Info)
    );
}
