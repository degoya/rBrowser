use rb_plugin_api::test_support::MemoryHost;
use rb_plugin_api::{Capability, Severity};

use super::*;

fn host() -> MemoryHost {
    let mut host = MemoryHost::new()
        .grant(Capability::ProjectRead)
        .grant(Capability::OutWrite);
    host.now = 1_731_300_000;
    host
}

#[test]
fn the_defaults_without_settings() {
    let settings = Settings::read(&mut host());
    assert_eq!(settings, Settings::default());
    assert!(settings.third_party && settings.since_navigation);
    assert!(!settings.reload_before_report);
}

#[test]
fn rules_ignore_and_switches_are_read_and_bad_values_skipped() {
    let mut host = host();
    host.settings.insert(
        "rules".to_owned(),
        r#"{"deprecation":"off","csp-violation":"error","x":"loud","y":3}"#.to_owned(),
    );
    host.settings.insert(
        "ignore".to_owned(),
        format!(
            r#"["*/ads/*", "  ", 7, "{}"]"#,
            "x".repeat(MAX_PATTERN_CHARS + 1)
        ),
    );
    host.settings
        .insert("reloadBeforeReport".to_owned(), "true".to_owned());
    host.settings
        .insert("thirdParty".to_owned(), "false".to_owned());
    host.settings
        .insert("sinceNavigation".to_owned(), "\"no\"".to_owned());
    let settings = Settings::read(&mut host);
    assert_eq!(settings.rules.len(), 2);
    assert_eq!(settings.rules["deprecation"], RuleSetting::Off);
    assert_eq!(
        settings.rules["csp-violation"],
        RuleSetting::Severity(Severity::Error)
    );
    assert_eq!(settings.ignore, vec!["*/ads/*".to_owned()]);
    assert!(settings.reload_before_report);
    assert!(!settings.third_party);
    assert!(settings.since_navigation);
}

#[test]
fn the_state_round_trips_through_the_out_folder() {
    let mut host = host();
    assert_eq!(State::read(&mut host), State::default());
    State {
        cleared_at: 42.5,
        pending: true,
    }
    .write(&mut host);
    let written = host.out.get(STATE_FILE).cloned().expect("written");
    host.project.insert(STATE_PATH.to_owned(), written);
    let state = State::read(&mut host);
    assert!((state.cleared_at - 42.5).abs() < f64::EPSILON);
    assert!(state.pending);
}

#[test]
fn a_state_that_cannot_be_written_is_logged() {
    let mut host = MemoryHost::new();
    State::default().write(&mut host);
    assert!(host.logs.iter().any(|(level, _)| level == "warn"));
}

#[test]
fn a_clear_mark_is_never_later_than_the_clock() {
    let mut host = host();
    let ceiling = (1_731_300_000.0 + 1.0) * 1000.0;
    assert!((clear_mark(&mut host, 5.0) - 5.0).abs() < f64::EPSILON);
    assert!((clear_mark(&mut host, 9e15) - ceiling).abs() < f64::EPSILON);
    assert!((clear_mark(&mut host, 0.0) - ceiling).abs() < f64::EPSILON);
}
