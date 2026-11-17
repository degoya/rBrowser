use std::collections::BTreeSet;

use rb_plugin_api::check::valid_rule_id;

use super::*;

#[test]
fn every_rule_is_described_once_with_its_own_title_key() {
    let described = describe();
    assert_eq!(described.len(), RULES.len());
    let ids: BTreeSet<&str> = described.iter().map(|rule| rule.id.as_str()).collect();
    assert_eq!(ids.len(), RULES.len(), "unique ids");
    for rule in &described {
        assert!(valid_rule_id(&rule.id), "{}", rule.id);
        assert_eq!(rule.title, format!("plugin.pwa-check.rules.{}", rule.id));
    }
}

#[test]
fn the_sink_keeps_the_rules_asked_for() {
    let enabled = vec![MANIFEST_MISSING.to_owned()];
    let mut sink = Sink::new(&enabled, "https://a.test/", Some(2));
    assert!(sink.wants(MANIFEST_MISSING));
    assert!(!sink.wants(SW_MISSING));
    sink.emit(SW_MISSING, Place::ServiceWorker, &[]);
    sink.emit(
        MANIFEST_MISSING,
        Place::Manifest,
        &[("url", "x".repeat(400))],
    );
    assert_eq!(sink.findings.len(), 1);
    let finding = &sink.findings[0];
    assert_eq!(finding.rule, MANIFEST_MISSING);
    assert_eq!(
        finding.message,
        "plugin.pwa-check.findings.manifest.missing"
    );
    assert_eq!(finding.severity, Severity::Error);
    assert_eq!(finding.view, Some(2));
    assert_eq!(finding.url.as_deref(), Some("https://a.test/"));
    assert_eq!(finding.selector, None);
    assert_eq!(finding.path.as_deref(), Some("pwa:manifest"));
    assert_eq!(finding.params[0].1.chars().count(), MAX_PARAM_CHARS + 3);
    let data: serde_json::Value =
        serde_json::from_str(finding.data.as_deref().expect("data")).expect("json");
    assert_eq!(data["panel"], "pwa");
    assert_eq!(data["section"], "manifest");
}

#[test]
fn every_rule_runs_when_the_run_names_none() {
    let mut sink = Sink::new(&[], "https://a.test/", None);
    assert!(RULES.iter().all(|rule| sink.wants(rule.id)));
    sink.emit("no.such_rule", Place::Manifest, &[]);
    assert!(sink.findings.is_empty(), "an undescribed rule is dropped");
}

#[test]
fn places_are_distinct_paths_with_their_panel_anchor() {
    let paths: BTreeSet<String> = [
        Place::Manifest,
        Place::Field("start_url"),
        Place::RequiredIcon(512),
        Place::Icon(3),
        Place::ServiceWorker,
        Place::Offline,
        Place::Install("no-manifest".to_owned()),
        Place::Meta("theme-color"),
    ]
    .iter()
    .map(Place::path)
    .collect();
    assert_eq!(paths.len(), 8);
    assert_eq!(Place::Icon(3).anchor(), "icon-3");
    assert_eq!(Place::Icon(3).section(), "icons");
    assert_eq!(Place::Install("x".to_owned()).section(), "install");
}

#[test]
fn every_rule_has_its_title_and_message_in_the_locale() {
    let locale: serde_json::Value =
        serde_json::from_str(include_str!("../locales/en.json")).expect("locale");
    let text = |key: &str| {
        key.split('.')
            .try_fold(&locale, |value, part| value.get(part))
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
    };
    for rule in RULES {
        for kind in ["rules", "findings"] {
            let key = crate::key(&format!("{kind}.{}", rule.id));
            assert!(text(&key).is_some(), "{key}");
        }
    }
    assert!(text("plugin.pwa-check.panels.pwa").is_some());
}
