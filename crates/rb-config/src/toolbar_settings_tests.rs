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
        json!({
            "overflow": "wrap",
            "panelsToggle": false,
            "portalButton": true
        })
    );
}

// RB-090-63: the toolbar's theme and footer buttons are gone (both in the main menu), and with
// them `footerToggle` and `themeToggle`. A settings file that still holds them loads as before:
// the keys are not read, the rest applies.
#[test]
fn a_file_with_the_removed_switches_still_loads() {
    let user: UserSettings = serde_json::from_value(json!({
        "ui": { "toolbar": {
            "overflow": "wrap",
            "footerToggle": false,
            "themeToggle": false,
            "portalButton": false
        } }
    }))
    .expect("settings");
    assert_eq!(user.ui.toolbar.overflow, ToolbarOverflow::Wrap);
    assert!(!user.ui.toolbar.portal_button);
    let written = serde_json::to_value(&user.ui.toolbar).expect("json");
    assert!(written.get("footerToggle").is_none(), "{written}");
    assert!(written.get("themeToggle").is_none(), "{written}");
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
    assert_eq!(user.ui.toolbar.overflow, ToolbarOverflow::Menu);
}

// RB-090-10, the migration: a settings file never stores a default it was not given (Settings,
// `settings_set` and the commands write the one key that changed, a reset removes it), so a person
// who kept the old default has no `panelsToggle` and follows the new one; a stored `true` is a
// choice of one's own and stays - read as `false`, the switch could never be turned on again.
#[test]
fn the_old_default_follows_the_new_one_and_a_choice_stays() {
    let kept_the_default: UserSettings =
        serde_json::from_value(json!({ "ui": { "toolbar": { "overflow": "menu" } } }))
            .expect("settings");
    assert!(!kept_the_default.ui.toolbar.panels_toggle);
    let chose_it: UserSettings =
        serde_json::from_value(json!({ "ui": { "toolbar": { "panelsToggle": true } } }))
            .expect("settings");
    assert!(chose_it.ui.toolbar.panels_toggle);
}

// RB-090-62: the portal's button in the toolbar is there unless switched off.
#[test]
fn the_toolbar_shows_the_portal_button_unless_told_not_to() {
    let user: UserSettings = serde_json::from_value(json!({})).expect("settings");
    assert!(user.ui.toolbar.portal_button);
    assert!(ToolbarSettings::default().portal_button);
    let user: UserSettings =
        serde_json::from_value(json!({ "ui": { "toolbar": { "portalButton": false } } }))
            .expect("settings");
    assert!(!user.ui.toolbar.portal_button);
    assert_eq!(user.ui.toolbar.overflow, ToolbarOverflow::Menu);
}
