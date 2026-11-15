use super::*;
use crate::ConfigError;
use crate::validate::validate;
use serde_json::{Value, json};

fn config(extra: Value) -> ProjectConfig {
    let mut value = json!({ "version": 1 });
    if let (Some(base), Some(extra)) = (value.as_object_mut(), extra.as_object()) {
        for (key, item) in extra {
            base.insert(key.clone(), item.clone());
        }
    }
    serde_json::from_value(value).expect("a config")
}

fn keys(issues: &[ConfigIssue]) -> Vec<(&str, &str)> {
    issues
        .iter()
        .map(|issue| (issue.path.as_str(), issue.key.as_str()))
        .collect()
}

#[test]
fn plain_ids_are_valid() {
    for id in [
        "default",
        "admin",
        "user-2",
        "qa_team",
        "a",
        "x1234567890123456789012345678901",
    ] {
        assert!(is_valid_session_id(id), "{id}");
    }
}

#[test]
fn ids_that_could_leave_their_folder_are_refused() {
    for id in [
        "",
        "..",
        ".",
        "../admin",
        "a/b",
        "a\\b",
        "C:",
        "admin.",
        " admin",
        "admin ",
        "-admin",
        "_admin",
        "Admin",
        "ADMIN",
        "a b",
        "a\u{0}b",
        "\u{e9}t\u{e9}",
        "x12345678901234567890123456789012",
    ] {
        assert!(!is_valid_session_id(id), "{id:?}");
    }
}

#[test]
fn windows_device_names_are_refused() {
    for id in ["con", "prn", "aux", "nul", "com1", "com9", "lpt1", "lpt9"] {
        assert!(!is_valid_session_id(id), "{id}");
    }
    assert!(is_valid_session_id("console"));
    assert!(is_valid_session_id("com10"));
}

#[test]
fn a_config_with_good_sessions_has_no_issues() {
    let config = config(json!({
        "sessions": ["default", "admin"],
        "sessionColors": { "admin": "#e5484d" }
    }));
    assert!(check_sessions(&config).is_empty());
}

#[test]
fn a_bad_or_repeated_session_id_is_an_issue() {
    let config = config(json!({ "sessions": ["admin", "../x", "admin"] }));
    assert_eq!(
        keys(&check_sessions(&config)),
        [
            ("/sessions/1", "config.error.invalid_session_id"),
            ("/sessions/2", "config.error.duplicate_id"),
        ]
    );
}

#[test]
fn session_colors_must_name_a_listed_session_and_be_hex() {
    let config = config(json!({
        "sessions": ["admin"],
        "sessionColors": { "admin": "red", "ghost": "#112233", "default": "#000000" }
    }));
    assert_eq!(
        keys(&check_sessions(&config)),
        [
            ("/sessionColors/admin", "config.error.session_color"),
            ("/sessionColors/default", "config.error.session_color"),
            ("/sessionColors/ghost", "config.error.unknown_session"),
        ]
    );
}

#[test]
fn session_colors_must_be_an_object() {
    let value = json!({ "version": 1, "sessions": ["admin"], "sessionColors": ["#112233"] });
    assert!(serde_json::from_value::<ProjectConfig>(value).is_err());
}

#[test]
fn session_colors_are_a_model_field() {
    let config = config(json!({ "sessions": ["admin"], "sessionColors": { "admin": "#e5484d" } }));
    assert_eq!(session_color(&config, "admin"), Some(0xe5_48_4d));
    assert_eq!(session_color(&config, "other"), None);
    assert!(!config.extra.contains_key(SESSION_COLORS_KEY));
}

#[test]
fn hex_colors() {
    assert_eq!(parse_hex_color("#e5484d"), Some(0xe5_48_4d));
    assert_eq!(parse_hex_color("#E5484D"), Some(0xe5_48_4d));
    for bad in ["e5484d", "#e5484", "#e5484dd", "#gggggg", "", "#"] {
        assert_eq!(parse_hex_color(bad), None, "{bad}");
    }
}

#[test]
fn the_validator_reports_session_issues() {
    let value = json!({ "version": 1, "sessions": ["Admin"] });
    match validate(value) {
        Err(ConfigError::Invalid(issues)) => assert_eq!(
            keys(&issues),
            [("/sessions/0", "config.error.invalid_session_id")]
        ),
        other => panic!("expected issues, got {other:?}"),
    }
}

#[test]
fn session_cookies_die_with_the_app_unless_the_user_keeps_them() {
    assert!(!SessionSettings::default().persist_session_cookies);
    let user: crate::UserSettings =
        serde_json::from_value(json!({ "version": 1 })).expect("user settings");
    assert!(!user.sessions.persist_session_cookies);
    let user: crate::UserSettings = serde_json::from_value(json!({
        "version": 1,
        "sessions": { "persistSessionCookies": true }
    }))
    .expect("user settings");
    assert!(user.sessions.persist_session_cookies);
}

#[test]
fn keeping_session_cookies_is_a_protected_user_key() {
    let def = crate::annotate::USER_KEYS
        .iter()
        .find(|def| def.key == "sessions.persistSessionCookies")
        .expect("a user key");
    assert!(def.protected, "never set over MCP");
}
