use std::collections::BTreeMap;

use super::*;
use crate::UserSettings;

#[test]
fn external_windows_default_to_beside_with_restart_and_tiles() {
    let settings: UserSettings = serde_json::from_str(r#"{ "version": 1 }"#).expect("parses");
    assert_eq!(
        settings.engines.external,
        ExternalWindowSettings {
            placement: "beside".into(),
            restart_on_crash: true,
            show_tiles: true,
            embed: false,
            scale_to_cell: true,
        }
    );
}

/// A Firefox or WebKit view is a window of its own unless `embed` is switched on (RB-080-39;
/// off by default since the owner's test of 2026-11-13).
#[test]
fn external_windows_are_embedded_in_their_cells_only_when_switched_on() {
    let json = r#"{ "version": 1, "engines": { "external": { "embed": true } } }"#;
    let settings: UserSettings = serde_json::from_str(json).expect("parses");
    assert!(settings.engines.external.embed);
    let written = serde_json::to_value(&settings.engines.external).expect("serializes");
    assert_eq!(written["embed"], true);
    let bare: UserSettings = serde_json::from_str(r#"{ "version": 1 }"#).expect("parses");
    assert!(!bare.engines.external.embed, "off by default");
}

#[test]
fn external_windows_read_in_camel_case() {
    let json = r#"{ "version": 1, "engines": { "external": {
        "placement": "monitor:2", "restartOnCrash": false, "showTiles": false, "embed": true,
        "scaleToCell": false
    } } }"#;
    let settings: UserSettings = serde_json::from_str(json).expect("parses");
    assert_eq!(settings.engines.external.placement, "monitor:2");
    assert!(!settings.engines.external.restart_on_crash);
    assert!(!settings.engines.external.show_tiles);
    assert!(settings.engines.external.embed);
    assert!(!settings.engines.external.scale_to_cell);
}

/// An embedded view's page is scaled to its cell unless `scaleToCell` is off (RB-080-40).
#[test]
fn embedded_views_are_scaled_to_their_cells_by_default() {
    let bare: UserSettings = serde_json::from_str(r#"{ "version": 1 }"#).expect("parses");
    assert!(bare.engines.external.scale_to_cell);
    let written = serde_json::to_value(&bare.engines.external).expect("serializes");
    assert_eq!(written["scaleToCell"], true);
}

/// A settings file from before 0.8 still reads: `autoRestart` is the old name (RB-080-38).
#[test]
fn the_old_auto_restart_key_still_reads() {
    let json = r#"{ "version": 1, "engines": { "external": { "autoRestart": false } } }"#;
    let settings: UserSettings = serde_json::from_str(json).expect("parses");
    assert!(!settings.engines.external.restart_on_crash);
    let written = serde_json::to_value(&settings.engines.external).expect("serializes");
    assert_eq!(written["restartOnCrash"], false);
    assert!(written.get("autoRestart").is_none());
}

#[test]
fn firefox_defaults_to_automatic_with_firefox_as_a_sync_source() {
    let settings: UserSettings = serde_json::from_str(r#"{ "version": 1 }"#).expect("parses");
    assert_eq!(
        settings.engines.firefox,
        FirefoxSettings {
            path: None,
            profile_dir: None,
            sync_source: true,
            extra_prefs: BTreeMap::new(),
        }
    );
    assert_eq!(settings.engines.firefox_executable(), None);
}

#[test]
fn firefox_reads_in_camel_case() {
    let json = r#"{ "version": 1, "engines": { "firefox": {
        "path": "C:\\FF\\firefox.exe", "profileDir": "D:\\ff", "syncSource": false,
        "extraPrefs": { "ui.prefersReducedMotion": 1 }
    } } }"#;
    let settings: UserSettings = serde_json::from_str(json).expect("parses");
    let firefox = &settings.engines.firefox;
    assert_eq!(firefox.profile_dir.as_deref(), Some("D:\\ff"));
    assert!(!firefox.sync_source);
    assert_eq!(
        firefox.extra_prefs.get("ui.prefersReducedMotion"),
        Some(&serde_json::json!(1))
    );
    assert_eq!(
        settings.engines.firefox_executable(),
        Some("C:\\FF\\firefox.exe")
    );
}

#[test]
fn the_old_firefox_path_is_read_but_never_written() {
    let json = r#"{ "version": 1, "engines": { "firefoxPath": "C:\\Old\\firefox.exe" } }"#;
    let settings: UserSettings = serde_json::from_str(json).expect("parses");
    assert_eq!(
        settings.engines.firefox_executable(),
        Some("C:\\Old\\firefox.exe")
    );
    let written = serde_json::to_string(&settings).expect("writes");
    assert!(!written.contains("firefoxPath"), "{written}");
    let both = r#"{ "version": 1, "engines": { "firefoxPath": "C:\\Old\\firefox.exe",
        "firefox": { "path": "C:\\New\\firefox.exe" } } }"#;
    let settings: UserSettings = serde_json::from_str(both).expect("parses");
    assert_eq!(
        settings.engines.firefox_executable(),
        Some("C:\\New\\firefox.exe")
    );
}

/// RB-090-54: one path for About and the start - the setting is trimmed once, here, so a path
/// with blanks at its edges is the same file for every reader (About, the start, the list).
#[test]
fn the_firefox_path_is_trimmed_for_every_reader() {
    let json =
        r#"{ "version": 1, "engines": { "firefox": { "path": "  C:\\FF\\firefox.exe \t" } } }"#;
    let settings: UserSettings = serde_json::from_str(json).expect("parses");
    assert_eq!(
        settings.engines.firefox_executable(),
        Some("C:\\FF\\firefox.exe")
    );
    let blank = r#"{ "version": 1, "engines": { "firefoxPath": "C:\\Old\\firefox.exe",
        "firefox": { "path": "   " } } }"#;
    let settings: UserSettings = serde_json::from_str(blank).expect("parses");
    assert_eq!(
        settings.engines.firefox_executable(),
        Some("C:\\Old\\firefox.exe")
    );
}

#[test]
fn webkit_is_on_and_a_sync_source_by_default() {
    let settings: UserSettings = serde_json::from_str(r#"{ "version": 1 }"#).expect("parses");
    assert_eq!(
        settings.engines.webkit,
        WebkitSettings {
            enabled: true,
            sync_source: true,
        }
    );
    let json =
        r#"{ "version": 1, "engines": { "webkit": { "enabled": false, "syncSource": false } } }"#;
    let settings: UserSettings = serde_json::from_str(json).expect("parses");
    assert!(!settings.engines.webkit.enabled);
    assert!(!settings.engines.webkit.sync_source);
}

#[test]
fn cookies_are_copied_by_hand_for_the_environments_domains_by_default() {
    let settings: UserSettings = serde_json::from_str(r#"{ "version": 1 }"#).expect("parses");
    assert_eq!(
        settings.engines.cookies,
        CookieSettings {
            copy: "manual".into(),
            domain_filter: "environment".into(),
            exclude: Vec::new(),
            reload: true,
        }
    );
}

#[test]
fn cookie_settings_read_in_camel_case() {
    let json = r#"{ "version": 1, "engines": { "cookies": {
        "copy": "onNavigate", "domainFilter": "all", "exclude": ["_ga*"], "reload": false
    } } }"#;
    let settings: UserSettings = serde_json::from_str(json).expect("parses");
    assert_eq!(settings.engines.cookies.copy, "onNavigate");
    assert_eq!(settings.engines.cookies.domain_filter, "all");
    assert_eq!(settings.engines.cookies.exclude, vec!["_ga*".to_owned()]);
    assert!(!settings.engines.cookies.reload);
}
