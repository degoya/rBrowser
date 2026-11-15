use super::*;
use serde_json::json;

#[test]
fn the_defaults_are_the_job_files() {
    let settings: PopOutSettings = serde_json::from_value(json!({})).expect("defaults");
    assert_eq!(settings, PopOutSettings::default());
    assert!(settings.keep_placeholder);
    assert!(settings.fit_to_screen);
    assert!(settings.remember_bounds);
}

#[test]
fn the_keys_are_camel_case() {
    let settings: PopOutSettings = serde_json::from_value(json!({
        "keepPlaceholder": false, "fitToScreen": false, "rememberBounds": false
    }))
    .expect("reads");
    assert!(!settings.keep_placeholder);
    assert!(!settings.fit_to_screen);
    assert!(!settings.remember_bounds);
}

#[test]
fn the_user_settings_carry_it_under_pop_out() {
    let user: crate::UserSettings =
        serde_json::from_value(json!({ "popOut": { "keepPlaceholder": false } })).expect("reads");
    assert!(!user.pop_out.keep_placeholder);
    assert!(user.pop_out.fit_to_screen);
}
