use super::*;
use crate::UserSettings;

/// RB-095-02: Android views are off until the person switches them on, and no adb is named.
#[test]
fn android_views_are_off_and_adb_is_found_by_default() {
    let settings: UserSettings = serde_json::from_str(r#"{ "version": 1 }"#).expect("parses");
    assert_eq!(settings.engines.android, AndroidSettings::default());
    assert!(!settings.engines.android.enabled);
    assert_eq!(settings.engines.android.adb_executable(), None);
}

#[test]
fn the_adb_path_is_read_trimmed_and_blank_is_unset() {
    let json = r#"{ "version": 1, "engines": { "android": {
        "enabled": true, "adbPath": "  C:\\Android\\platform-tools\\adb.exe " } } }"#;
    let settings: UserSettings = serde_json::from_str(json).expect("parses");
    assert!(settings.engines.android.enabled);
    assert_eq!(
        settings.engines.android.adb_executable(),
        Some("C:\\Android\\platform-tools\\adb.exe")
    );
    let blank = AndroidSettings {
        enabled: true,
        adb_path: Some("   ".to_owned()),
        ..AndroidSettings::default()
    };
    assert_eq!(blank.adb_executable(), None);
}

#[test]
fn an_unset_adb_path_is_not_written() {
    let written = serde_json::to_value(AndroidSettings::default()).expect("serializes");
    assert_eq!(
        written,
        serde_json::json!({ "enabled": false, "reverseLocalhost": true })
    );
}

/// RB-096-12 (owner, 2026-11-21): a loopback address of a view reaches this machine's port by
/// default; the person can switch it off.
#[test]
fn localhost_reaches_the_pc_by_default_and_can_be_switched_off() {
    let settings: UserSettings = serde_json::from_str(r#"{ "version": 1 }"#).expect("parses");
    assert!(settings.engines.android.reverse_localhost);
    let off = r#"{ "version": 1, "engines": { "android": { "reverseLocalhost": false } } }"#;
    let settings: UserSettings = serde_json::from_str(off).expect("parses");
    assert!(!settings.engines.android.reverse_localhost);
    assert!(
        !settings.engines.android.enabled,
        "the other keys keep their defaults"
    );
}
