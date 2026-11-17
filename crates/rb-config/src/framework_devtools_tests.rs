use super::*;
use serde_json::json;

use crate::UserSettings;

#[test]
fn both_frameworks_are_offered_by_default() {
    let settings: FrameworkDevtoolsSettings = serde_json::from_value(json!({})).expect("defaults");
    assert_eq!(settings, FrameworkDevtoolsSettings::default());
    assert_eq!(settings.vue, FrameworkDevtoolsMode::Auto);
    assert_eq!(settings.react, FrameworkDevtoolsMode::Auto);
    assert!(settings.probes());
}

#[test]
fn the_modes_are_lower_case_words() {
    let settings: FrameworkDevtoolsSettings =
        serde_json::from_value(json!({ "vue": "on", "react": "off" })).expect("reads");
    assert_eq!(settings.vue, FrameworkDevtoolsMode::On);
    assert_eq!(settings.react, FrameworkDevtoolsMode::Off);
    assert!(settings.probes());
    assert_eq!(
        serde_json::to_value(settings).expect("serializes"),
        json!({ "vue": "on", "react": "off" })
    );
    assert!(
        serde_json::from_value::<FrameworkDevtoolsSettings>(json!({ "vue": "maybe" })).is_err()
    );
}

#[test]
fn nothing_is_probed_with_both_off() {
    let settings = FrameworkDevtoolsSettings {
        vue: FrameworkDevtoolsMode::Off,
        react: FrameworkDevtoolsMode::Off,
    };
    assert!(!settings.probes());
    assert!(!FrameworkDevtoolsMode::Off.is_on());
    assert!(FrameworkDevtoolsMode::Auto.is_on());
}

#[test]
fn the_user_settings_carry_them_under_framework_devtools() {
    let user: UserSettings =
        serde_json::from_value(json!({ "frameworkDevtools": { "react": "on" } })).expect("reads");
    assert_eq!(user.framework_devtools.react, FrameworkDevtoolsMode::On);
    assert_eq!(user.framework_devtools.vue, FrameworkDevtoolsMode::Auto);
    let json = serde_json::to_value(&user).expect("serializes");
    assert_eq!(json["frameworkDevtools"]["react"], "on");
}
