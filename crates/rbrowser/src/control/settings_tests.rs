use std::collections::BTreeMap;

use super::*;

#[test]
fn the_registry_takes_both_schemas_whole() {
    let (registry, issues) = settings_registry();
    assert!(issues.is_empty(), "{issues:?}");
    for key in [
        "startUrl",
        "sync.scroll",
        "ui.theme",
        "logs.level",
        "updates.cefAgeWarning",
        "shortcuts.bindings",
        "plugins.crashLimit",
    ] {
        assert!(registry.get(key).is_some(), "{key}");
    }
    for protected in [
        "engines.chromium.sandbox",
        "plugins.disabled",
        "plugins.trustedKeys",
        "plugins.developmentMode",
        "mcp.enabled",
        "logs.level",
    ] {
        assert!(
            registry.check_mcp_access(protected).is_err(),
            "{protected} is open to MCP"
        );
    }
    assert!(registry.check_mcp_access("logs.maxFiles").is_ok());
}

#[test]
fn the_log_settings_of_both_crates_agree() {
    // rb-config's schema defines the `logs` keys; rb-diag reads them with its own defaults.
    assert_eq!(
        log_settings(&UserSettings::default()),
        rb_diag::LogSettings::default()
    );
    let mut user = UserSettings::default();
    user.logs.level = "debug".to_owned();
    user.logs.max_files = 7;
    let read = log_settings(&user);
    assert_eq!(read.level, rb_diag::LogLevel::Debug);
    assert_eq!(read.max_files, 7);
    let schema = rb_config::user_settings_schema();
    let levels: Vec<&str> = rb_diag::LogLevel::ALL.iter().map(|l| l.as_str()).collect();
    assert_eq!(
        schema.as_value()["properties"]["logs"]["properties"]["level"]["enum"],
        serde_json::json!(levels)
    );
    user.logs.level = "loud".to_owned();
    assert_eq!(log_settings(&user), rb_diag::LogSettings::default());
}

#[test]
fn the_leader_follows_the_shortcut_settings() {
    let mut user = UserSettings::default();
    assert_eq!(chord_settings(&user), ChordSettings::default());
    assert_eq!(
        chord_settings(&user).leader_timeout,
        None,
        "0 ms: no timeout"
    );
    user.shortcuts.leader_timeout_ms = 900;
    user.shortcuts.disable_in_devtools = false;
    let settings = chord_settings(&user);
    assert_eq!(settings.leader_timeout, Some(Duration::from_millis(900)));
    assert!(!settings.disable_in_devtools);
}

#[test]
fn header_values_are_masked_in_the_log() {
    let mut config = ProjectConfig::default();
    config.emulation.headers =
        BTreeMap::from([("X-Api-Key".to_owned(), "core-settings-secret-1".to_owned())]);
    register_config_secrets(&config);
    assert!(!rb_diag::redact("key core-settings-secret-1").contains("core-settings-secret-1"));
}

#[test]
fn a_value_out_of_range_falls_back_and_is_reported() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("settings.json");
    std::fs::write(&path, r#"{ "shortcuts": { "leaderTimeoutMs": 20000 } }"#).expect("write");
    let user = load_user_from(path);
    assert!(user.error.is_none());
    assert_eq!(user.warnings.len(), 1);
    assert_eq!(
        chord_settings(&user.settings).leader_timeout,
        None,
        "the default applies: a chord can still be typed"
    );
}

#[test]
fn the_user_values_carry_their_origin() {
    let (registry, _) = settings_registry();
    let mut user = UserSettings::default();
    user.ui.theme = rb_config::Theme::Dark;
    let values = user_values(&registry, &user);
    let find = |key: &str| values.iter().find(|value| value.key == key);
    let theme = find("ui.theme").expect("ui.theme");
    assert_eq!(theme.value, serde_json::json!("dark"));
    assert_eq!(theme.origin, rb_config::Origin::User);
    let timeout = find("shortcuts.leaderTimeoutMs").expect("leaderTimeoutMs");
    assert_eq!(timeout.value, serde_json::json!(0));
    assert_eq!(timeout.origin, rb_config::Origin::Default);
    assert!(
        find("sync.scroll").is_none(),
        "project keys are not the user's"
    );
}

/// RB-094-26: a plugin key the user file does not set comes back with its default as the
/// default, as the settings window resolves it - not as `null` set by the user.
#[test]
fn an_unset_plugin_key_is_its_default() {
    let schema = serde_json::from_str(include_str!(
        "../../../../plugins/api-client/settings.schema.json"
    ))
    .expect("the API client's settings schema is JSON");
    let registry = super::super::plugin_keys::with_plugins(&[("api-client".to_owned(), schema)]);
    let find = |values: &[SettingValue], key: &str| {
        values
            .iter()
            .find(|value| value.key == key)
            .cloned()
            .expect("the plugin's key")
    };
    let unset = find(
        &user_values(&registry, &UserSettings::default()),
        "api-client.benchRepeats",
    );
    assert_eq!(unset.value, serde_json::json!(10));
    assert_eq!(unset.origin, rb_config::Origin::Default);

    let mut user = UserSettings::default();
    user.extra.insert(
        "api-client".to_owned(),
        serde_json::json!({ "benchRepeats": 20 }),
    );
    let set = find(&user_values(&registry, &user), "api-client.benchRepeats");
    assert_eq!(set.value, serde_json::json!(20));
    assert_eq!(set.origin, rb_config::Origin::User);
}
