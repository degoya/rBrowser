use super::*;
use serde_json::json;

use crate::{ChromiumSettings, UserSettings};

#[test]
fn the_defaults_are_the_job_files() {
    let settings: ChromiumExtensionSettings = serde_json::from_value(json!({})).expect("defaults");
    assert_eq!(settings, ChromiumExtensionSettings::default());
    assert!(settings.enabled);
    assert!(settings.offer_project_recommendations);
    assert!(settings.unpacked.is_empty());
    // Web Store installs (RB-080-23): allowed; the check for updates at the start asks Google
    // for every installed extension, so it is the person's choice.
    assert!(settings.allow_web_store);
    assert!(!settings.check_updates_at_start);
}

#[test]
fn the_web_store_keys_are_camel_case() {
    let settings: ChromiumExtensionSettings = serde_json::from_value(json!({
        "allowWebStore": false,
        "checkUpdatesAtStart": true,
    }))
    .expect("reads");
    assert!(!settings.allow_web_store);
    assert!(settings.check_updates_at_start);
    let written = serde_json::to_value(&settings).expect("writes");
    assert_eq!(written["allowWebStore"], false);
    assert_eq!(written["checkUpdatesAtStart"], true);
}

#[test]
fn the_keys_are_camel_case_and_an_entry_is_enabled_unless_it_says_otherwise() {
    let settings: ChromiumExtensionSettings = serde_json::from_value(json!({
        "enabled": false,
        "offerProjectRecommendations": false,
        "unpacked": [
            { "path": "C:\\dev\\vue-devtools" },
            { "path": "C:\\dev\\hello", "enabled": false }
        ]
    }))
    .expect("reads");
    assert!(!settings.enabled);
    assert!(!settings.offer_project_recommendations);
    assert_eq!(
        settings.unpacked,
        vec![
            UnpackedExtension {
                path: "C:\\dev\\vue-devtools".to_owned(),
                enabled: true,
            },
            UnpackedExtension {
                path: "C:\\dev\\hello".to_owned(),
                enabled: false,
            },
        ]
    );
}

#[test]
fn an_entry_without_a_path_does_not_read() {
    let read = serde_json::from_value::<ChromiumExtensionSettings>(json!({
        "unpacked": [{ "enabled": true }]
    }));
    assert!(read.is_err());
}

#[test]
fn the_extensions_sit_under_engines_chromium_and_are_left_out_while_default() {
    let user = UserSettings::default();
    let written = serde_json::to_value(&user).expect("writes");
    assert!(
        written["engines"]["chromium"].get("extensions").is_none(),
        "{written}"
    );
    let chromium: ChromiumSettings = serde_json::from_value(json!({
        "extensions": { "unpacked": [{ "path": "D:\\ext" }] }
    }))
    .expect("reads");
    assert!(chromium.sandbox, "the sandbox keeps its default");
    assert_eq!(chromium.extensions.unpacked.len(), 1);
    let written = serde_json::to_value(&chromium).expect("writes");
    assert_eq!(written["extensions"]["unpacked"][0]["path"], "D:\\ext");
}

#[test]
fn enabled_paths_are_the_switched_on_entries_of_a_switched_on_list() {
    let mut settings = ChromiumExtensionSettings {
        unpacked: vec![
            UnpackedExtension {
                path: "A".to_owned(),
                enabled: true,
            },
            UnpackedExtension {
                path: "B".to_owned(),
                enabled: false,
            },
        ],
        ..ChromiumExtensionSettings::default()
    };
    assert_eq!(settings.enabled_paths().collect::<Vec<_>>(), ["A"]);
    settings.enabled = false;
    assert_eq!(settings.enabled_paths().count(), 0);
}
