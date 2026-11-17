use super::*;

#[test]
fn an_empty_file_gives_the_defaults() {
    let settings: UserSettings = serde_json::from_str(r#"{ "version": 1 }"#).expect("parses");
    assert_eq!(settings.ui.language, "en");
    assert_eq!(settings.ui.theme, Theme::System);
    assert!(!settings.cdp.enabled);
    assert!(settings.shortcuts.disable_in_devtools);
    assert!(settings.shortcuts.bindings.is_empty());
    assert!(!settings.devices.replace_defaults);
    assert!(settings.updates.cef_age_warning);
    assert_eq!(settings.logs.level, "info");
    assert_eq!(
        (
            settings.logs.max_file_size_mb,
            settings.logs.max_files,
            settings.logs.crash_dumps_in_bundle
        ),
        (10, 5, 3)
    );
}

#[test]
fn logs_and_the_cef_age_warning_read_in_camel_case() {
    let json = r#"{
        "version": 1,
        "updates": { "cefAgeWarning": false },
        "logs": { "level": "debug", "maxFileSizeMb": 20, "maxFiles": 2, "crashDumpsInBundle": 0 }
    }"#;
    let settings: UserSettings = serde_json::from_str(json).expect("parses");
    assert!(!settings.updates.cef_age_warning);
    assert_eq!(settings.updates.channel, UpdateChannel::Stable);
    assert_eq!(settings.logs.level, "debug");
    assert_eq!(settings.logs.max_file_size_mb, 20);
    assert_eq!(settings.logs.max_files, 2);
    assert_eq!(settings.logs.crash_dumps_in_bundle, 0);
}

#[test]
fn keys_are_camel_case_and_unknown_sections_survive() {
    let json = r#"{
        "version": 1,
        "mcp": { "enabled": true, "allowEvaluate": false },
        "devtools": { "mode": "docked-right" },
        "shortcuts": { "leaderTimeoutMs": 900, "bindings": { "sync.toggle": ["Ctrl+Shift+Space S S"] } },
        "breakpointShortcuts": { "newViewModifier": "Shift", "overlayMs": 800 },
        "devices": { "replaceDefaults": true, "items": [] },
        "my-plugin": 1
    }"#;
    let settings: UserSettings = serde_json::from_str(json).expect("parses");
    assert!(settings.mcp.enabled);
    assert_eq!(settings.devtools.mode, DevtoolsMode::DockedRight);
    assert_eq!(settings.shortcuts.leader_timeout_ms, 900);
    assert_eq!(settings.shortcuts.bindings["sync.toggle"].len(), 1);
    assert_eq!(
        settings.breakpoint_shortcuts.new_view_modifier,
        Some(NewViewModifier::Shift)
    );
    assert!(settings.devices.replace_defaults);
    assert_eq!(settings.extra["my-plugin"], serde_json::json!(1));
}

#[test]
fn the_default_cell_zoom_is_fit_or_a_factor() {
    let settings: UserSettings = serde_json::from_str(r#"{ "version": 1 }"#).expect("parses");
    assert_eq!(settings.grid.default_zoom, crate::Zoom::Fit);
    let json = r#"{ "version": 1, "grid": { "defaultZoom": 0.5 } }"#;
    let settings: UserSettings = serde_json::from_str(json).expect("parses");
    assert_eq!(settings.grid.default_zoom, crate::Zoom::Factor(0.5));
    let back = serde_json::to_value(&settings).expect("serializes");
    assert_eq!(back["grid"]["defaultZoom"], serde_json::json!(0.5));
    assert!(back["grid"].get("spacePan").is_none());
}

#[test]
fn the_new_view_modifier_is_shift_or_ctrl_never_alt() {
    let parse = |value: &str| {
        let json = format!(r#"{{ "breakpointShortcuts": {{ "newViewModifier": "{value}" }} }}"#);
        serde_json::from_str::<UserSettings>(&json)
            .map(|settings| settings.breakpoint_shortcuts.new_view_modifier)
    };
    assert_eq!(parse("Shift").expect("Shift"), Some(NewViewModifier::Shift));
    assert_eq!(parse("Ctrl").expect("Ctrl"), Some(NewViewModifier::Ctrl));
    // Alt with the Ctrl of a breakpoint key is AltGr on German, French and Spanish keyboards.
    assert!(parse("Alt").is_err());
    assert!(parse("Win").is_err());
}

/// Self-update is off until a person switches it on (RB-090-02); the file reads the new keys.
#[test]
fn self_update_is_off_by_default_and_its_keys_read_back() {
    let settings = UserSettings::default();
    assert!(!settings.updates.enabled);
    assert_eq!(settings.updates.check, UpdateCheck::Off);
    assert_eq!(settings.updates.auto_install, UpdateAutoInstall::Off);
    let read: UserSettings = serde_json::from_value(serde_json::json!({
        "updates": { "enabled": true, "check": "onStart", "autoInstall": "onQuit" }
    }))
    .expect("reads");
    assert!(read.updates.enabled);
    assert_eq!(read.updates.check, UpdateCheck::OnStart);
    assert_eq!(read.updates.auto_install, UpdateAutoInstall::OnQuit);
    assert!(read.updates.cef_age_warning, "the rest defaults");
}

#[test]
fn the_page_menu_has_its_emulation_block_unless_the_setting_turns_it_off() {
    let settings: UserSettings = serde_json::from_str(r#"{ "version": 1 }"#).expect("parses");
    assert!(settings.ui.page_context_menu_emulation);
    let json = r#"{ "version": 1, "ui": { "pageContextMenuEmulation": false } }"#;
    let settings: UserSettings = serde_json::from_str(json).expect("parses");
    assert!(!settings.ui.page_context_menu_emulation);
}

#[test]
fn inspect_opens_devtools_unless_the_setting_turns_it_off() {
    let settings: UserSettings = serde_json::from_str(r#"{ "version": 1 }"#).expect("parses");
    assert!(settings.devtools.open_on_inspect);
    let json = r#"{ "version": 1, "devtools": { "openOnInspect": false } }"#;
    let settings: UserSettings = serde_json::from_str(json).expect("parses");
    assert!(!settings.devtools.open_on_inspect);
}

#[test]
fn the_devtools_screencast_is_off_unless_the_setting_turns_it_on() {
    let settings: UserSettings = serde_json::from_str(r#"{ "version": 1 }"#).expect("parses");
    assert!(!settings.devtools.screencast);
    let json = r#"{ "version": 1, "devtools": { "screencast": true } }"#;
    let settings: UserSettings = serde_json::from_str(json).expect("parses");
    assert!(settings.devtools.screencast);
}

#[test]
fn the_devtools_dock_size_is_a_setting_with_defaults() {
    let settings: UserSettings = serde_json::from_str(r#"{ "version": 1 }"#).expect("parses");
    assert_eq!(
        (settings.devtools.dock_height, settings.devtools.dock_width),
        (320, 480)
    );
    let json = r#"{ "version": 1, "devtools": { "dockHeight": 400, "dockWidth": 640 } }"#;
    let settings: UserSettings = serde_json::from_str(json).expect("parses");
    assert_eq!(
        (settings.devtools.dock_height, settings.devtools.dock_width),
        (400, 640)
    );
}

#[test]
fn the_engine_badge_is_on_by_default_and_switchable() {
    let settings: UserSettings = serde_json::from_str(r#"{ "version": 1 }"#).expect("parses");
    assert!(
        settings.ui.engine_badge,
        "RB-080-33: the badge shows by default"
    );
    let off: UserSettings =
        serde_json::from_str(r#"{ "version": 1, "ui": { "engineBadge": false } }"#)
            .expect("parses");
    assert!(!off.ui.engine_badge);
}

#[test]
fn the_json_views_syntax_colours_are_on_by_default_and_switchable() {
    let settings: UserSettings = serde_json::from_str(r#"{ "version": 1 }"#).expect("parses");
    assert!(
        settings.settings.json_highlight,
        "RB-080-42: the settings window's JSON view colours by default"
    );
    let off: UserSettings =
        serde_json::from_str(r#"{ "version": 1, "settings": { "jsonHighlight": false } }"#)
            .expect("parses");
    assert!(!off.settings.json_highlight);
}

#[test]
fn the_settings_window_hides_keys_and_shows_tabs_by_default() {
    let settings: UserSettings = serde_json::from_str(r#"{ "version": 1 }"#).expect("parses");
    assert!(
        !settings.settings.show_keys,
        "RB-080-70: the key is in the title's tooltip, not under every title"
    );
    assert!(
        settings.settings.section_tabs,
        "RB-080-71: a category of several sections shows them as tabs"
    );
    let changed: UserSettings = serde_json::from_str(
        r#"{ "version": 1, "settings": { "showKeys": true, "sectionTabs": false } }"#,
    )
    .expect("parses");
    assert!(changed.settings.show_keys);
    assert!(!changed.settings.section_tabs);
}

#[test]
fn the_view_context_menu_is_on_by_default_and_switchable() {
    let settings: UserSettings = serde_json::from_str(r#"{ "version": 1 }"#).expect("parses");
    assert!(
        settings.ui.view_context_menu,
        "RB-080-41: the menus of the label strips and the canvas show by default"
    );
    let off: UserSettings =
        serde_json::from_str(r#"{ "version": 1, "ui": { "viewContextMenu": false } }"#)
            .expect("parses");
    assert!(!off.ui.view_context_menu);
}

#[test]
fn new_bookmarks_follow_the_environment_by_default_and_switchably() {
    let settings: UserSettings = serde_json::from_str(r#"{ "version": 1 }"#).expect("parses");
    assert!(
        settings.bookmarks.follow_environment,
        "RB-080-48: a page of an environment is bookmarked as its path by default"
    );
    let off: UserSettings =
        serde_json::from_str(r#"{ "version": 1, "bookmarks": { "followEnvironment": false } }"#)
            .expect("parses");
    assert!(!off.bookmarks.follow_environment);
}

#[test]
fn the_new_size_picker_lists_the_devices_by_default_and_switchable() {
    let settings = UserSettings::default();
    assert!(
        settings.ui.new_view_size.devices,
        "RB-080-46: on by default"
    );
    let off: UserSettings =
        serde_json::from_str(r#"{ "version": 1, "ui": { "newViewSize": { "devices": false } } }"#)
            .expect("parses");
    assert!(!off.ui.new_view_size.devices);
}

#[test]
fn syntax_colours_are_on_by_default_and_switchable() {
    let settings: UserSettings = serde_json::from_str(r#"{ "version": 1 }"#).expect("parses");
    assert!(
        settings.ui.syntax_highlight,
        "RB-080-65: code shows in syntax colours by default"
    );
    let off: UserSettings =
        serde_json::from_str(r#"{ "version": 1, "ui": { "syntaxHighlight": false } }"#)
            .expect("parses");
    assert!(!off.ui.syntax_highlight);
}

#[test]
fn the_shell_follows_windows_text_size_by_default_and_can_stop() {
    let settings: UserSettings = serde_json::from_str(r#"{ "version": 1 }"#).expect("parses");
    assert!(
        settings.ui.follow_windows_text_size,
        "RB-090-26: the shell grows with Windows' text size by default"
    );
    let off: UserSettings =
        serde_json::from_str(r#"{ "version": 1, "ui": { "followWindowsTextSize": false } }"#)
            .expect("parses");
    assert!(!off.ui.follow_windows_text_size);
}
