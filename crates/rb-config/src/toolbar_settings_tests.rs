use serde_json::json;

use super::*;
use crate::UserSettings;

#[test]
fn the_toolbar_overflows_into_the_menu_unless_told_to_wrap() {
    let user: UserSettings = serde_json::from_value(json!({})).expect("settings");
    assert_eq!(user.ui.toolbar.overflow, ToolbarOverflow::Menu);
    let user: UserSettings =
        serde_json::from_value(json!({ "ui": { "toolbar": { "overflow": "wrap" } } }))
            .expect("settings");
    assert_eq!(user.ui.toolbar.overflow, ToolbarOverflow::Wrap);
    assert_eq!(
        serde_json::to_value(&user.ui.toolbar).expect("json"),
        json!({ "overflow": "wrap", "footerToggle": true, "themeToggle": true, "panelsToggle": false })
    );
}

// RB-080-60: the footer's switch in the toolbar is there unless switched off.
#[test]
fn the_toolbar_shows_the_footer_switch_unless_told_not_to() {
    let user: UserSettings = serde_json::from_value(json!({})).expect("settings");
    assert!(user.ui.toolbar.footer_toggle);
    assert!(ToolbarSettings::default().footer_toggle);
    let user: UserSettings =
        serde_json::from_value(json!({ "ui": { "toolbar": { "footerToggle": false } } }))
            .expect("settings");
    assert!(!user.ui.toolbar.footer_toggle);
    assert_eq!(user.ui.toolbar.overflow, ToolbarOverflow::Menu);
}

// RB-080-95: the switch of rBrowser's own theme in the toolbar is there unless switched off.
#[test]
fn the_toolbar_shows_the_theme_switch_unless_told_not_to() {
    let user: UserSettings = serde_json::from_value(json!({})).expect("settings");
    assert!(user.ui.toolbar.theme_toggle);
    assert!(ToolbarSettings::default().theme_toggle);
    let user: UserSettings =
        serde_json::from_value(json!({ "ui": { "toolbar": { "themeToggle": false } } }))
            .expect("settings");
    assert!(!user.ui.toolbar.theme_toggle);
    assert!(user.ui.toolbar.footer_toggle);
}

#[test]
fn anything_else_is_refused() {
    let user: Result<UserSettings, _> =
        serde_json::from_value(json!({ "ui": { "toolbar": { "overflow": "scroll" } } }));
    assert!(user.is_err());
}

#[test]
fn only_wrap_gives_the_toolbar_a_second_row() {
    assert_eq!(ToolbarOverflow::Menu.max_rows(), 1);
    assert_eq!(ToolbarOverflow::Wrap.max_rows(), 2);
}

#[test]
fn the_schema_names_both_values() {
    let schema = serde_json::to_string(&schemars::schema_for!(ToolbarSettings)).expect("schema");
    assert!(schema.contains("\"menu\""), "{schema}");
    assert!(schema.contains("\"wrap\""), "{schema}");
}

// RB-080-79, RB-090-10: the panels' switch in the toolbar is off unless switched on - the icon
// rail beside the views opens the panels.
#[test]
fn the_toolbar_shows_the_panels_switch_only_when_told_to() {
    let user: UserSettings = serde_json::from_value(json!({})).expect("settings");
    assert!(!user.ui.toolbar.panels_toggle);
    assert!(!ToolbarSettings::default().panels_toggle);
    let user: UserSettings =
        serde_json::from_value(json!({ "ui": { "toolbar": { "panelsToggle": false } } }))
            .expect("settings");
    assert!(!user.ui.toolbar.panels_toggle);
    assert!(user.ui.toolbar.footer_toggle);
}

// RB-090-10, the migration: a settings file never stores a default it was not given (Settings,
// `settings_set` and the commands write the one key that changed, a reset removes it), so a person
// who kept the old default has no `panelsToggle` and follows the new one; a stored `true` is a
// choice of one's own and stays - read as `false`, the switch could never be turned on again.
#[test]
fn the_old_default_follows_the_new_one_and_a_choice_stays() {
    let kept_the_default: UserSettings =
        serde_json::from_value(json!({ "ui": { "toolbar": { "footerToggle": true } } }))
            .expect("settings");
    assert!(!kept_the_default.ui.toolbar.panels_toggle);
    let chose_it: UserSettings =
        serde_json::from_value(json!({ "ui": { "toolbar": { "panelsToggle": true } } }))
            .expect("settings");
    assert!(chose_it.ui.toolbar.panels_toggle);
}
