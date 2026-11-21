use rb_plugin_api::Severity;
use rb_plugin_api::check::{MAX_RULES, valid_rule_id};
use serde_json::json;

use super::{all, builtin_config, describe, find, html_validate_version, is_nu, wanted};
use crate::settings::Settings;

fn every() -> Vec<String> {
    all().iter().map(|rule| rule.id.clone()).collect()
}

#[test]
fn the_table_holds_the_standard_rules_and_the_nu_types() {
    assert_eq!(html_validate_version(), "11.18.0");
    assert!(all().len() > 30 && all().len() <= MAX_RULES);
    for id in [
        "no-dup-id",
        "element-permitted-content",
        "close-order",
        "missing-doctype",
        "no-unknown-attributes",
        "nu-error",
        "nu-warning",
        "nu-info",
    ] {
        assert!(find(id).is_some(), "{id}");
    }
    // Style rules and html-validate's directive rules stay out.
    for id in [
        "attr-quotes",
        "no-inline-style",
        "deprecated-rule",
        "no-unused-disable",
    ] {
        assert!(find(id).is_none(), "{id}");
    }
    for rule in all() {
        assert!(valid_rule_id(&rule.id), "{}", rule.id);
    }
}

#[test]
fn describes_every_rule_with_its_title_key_and_help_page() {
    let described = describe();
    assert_eq!(described.len(), all().len());
    let dup = described
        .iter()
        .find(|rule| rule.id == "no-dup-id")
        .expect("no-dup-id");
    assert_eq!(dup.title, "plugin.html-validate.rules.no-dup-id");
    assert_eq!(dup.severity, Severity::Error);
    assert_eq!(
        dup.help_url.as_deref(),
        Some("https://html-validate.org/rules/no-dup-id.html")
    );
    let nu = described
        .iter()
        .find(|rule| rule.id == "nu-info")
        .expect("nu-info");
    assert_eq!(nu.severity, Severity::Info);
    assert_eq!(nu.help_url, None);
}

#[test]
fn the_builtin_config_keeps_the_options_and_leaves_out_the_nu_rules() {
    let config = builtin_config(&Settings::default(), &every());
    assert_eq!(config.get("no-dup-id"), Some(&json!("error")));
    assert_eq!(
        config.get("no-raw-characters"),
        Some(&json!(["error", { "relaxed": true }]))
    );
    assert!(config.keys().all(|id| !is_nu(id)));
    // Every html-validate rule but the opt-in one, off until a project switches it on.
    assert_eq!(
        config.len(),
        all().iter().filter(|r| !is_nu(&r.id)).count() - 1
    );
    assert!(!config.contains_key("no-unknown-attributes"));
}

#[test]
fn the_project_rule_file_sets_levels_and_options_over_the_table() {
    let rules = crate::project::parse(
        br#"{"rules": {
            "no-unknown-attributes": "error",
            "no-dup-id": "warn",
            "close-order": "off",
            "no-raw-characters": ["error", {"relaxed": false}]
        }}"#,
    )
    .expect("rules");
    let settings = Settings {
        disabled_rules: vec!["no-dup-id".to_owned()],
        project: crate::project::ProjectRules::applied(rules),
        ..Settings::default()
    };
    let config = builtin_config(&settings, &every());
    assert_eq!(config.get("no-unknown-attributes"), Some(&json!("error")));
    // The file switches on what the settings switched off, at its own level.
    assert_eq!(config.get("no-dup-id"), Some(&json!("warn")));
    assert!(!config.contains_key("close-order"));
    assert_eq!(
        config.get("no-raw-characters"),
        Some(&json!(["error", { "relaxed": false }]))
    );
    // The lowest severity still holds for the file's levels.
    let strict = Settings {
        min_severity: Severity::Error,
        ..settings
    };
    assert!(!builtin_config(&strict, &every()).contains_key("no-dup-id"));
}

#[test]
fn the_builtin_config_leaves_out_what_is_switched_off() {
    let settings = Settings {
        disabled_rules: vec!["no-dup-id".to_owned()],
        ..Settings::default()
    };
    let enabled = vec!["no-dup-id".to_owned(), "close-order".to_owned()];
    let config = builtin_config(&settings, &enabled);
    assert_eq!(config.keys().collect::<Vec<_>>(), ["close-order"]);
}

#[test]
fn a_rule_is_wanted_only_when_left_on_and_not_disabled() {
    let settings = Settings {
        disabled_rules: vec!["nu-info".to_owned()],
        ..Settings::default()
    };
    let enabled = every();
    assert!(wanted("nu-error", &settings, &enabled));
    assert!(!wanted("nu-info", &settings, &enabled));
    assert!(!wanted("no-dup-id", &settings, &["close-order".to_owned()]));
}
