use super::*;
use serde_json::json;

#[test]
fn the_defaults_are_the_job_files() {
    let settings: LayoutModeSettings = serde_json::from_value(json!({})).expect("defaults");
    assert_eq!(settings, LayoutModeSettings::default());
    assert_eq!(settings.snap, 8);
    assert!(settings.minimap);
    assert_eq!(settings.minimap_position, MinimapPosition::End);
    assert_eq!(settings.horizontal_height, StripHeight::Fit);
    assert_eq!(settings.split_step, 0.05);
    assert!(settings.space_pan);
    assert_eq!(settings.save_preset_to, PresetFile::Local);
}

#[test]
fn the_keys_are_camel_case() {
    let settings: LayoutModeSettings = serde_json::from_value(json!({
        "snap": 16, "minimap": false, "minimapPosition": "start",
        "horizontalHeight": 600, "splitStep": 0.1, "spacePan": false,
        "savePresetTo": "project"
    }))
    .expect("reads");
    assert_eq!(settings.snap, 16);
    assert!(!settings.minimap);
    assert_eq!(settings.minimap_position, MinimapPosition::Start);
    assert_eq!(settings.horizontal_height, StripHeight::Px(600));
    assert_eq!(settings.split_step, 0.1);
    assert!(!settings.space_pan);
    assert_eq!(settings.save_preset_to, PresetFile::Project);
}

#[test]
fn the_preset_file_names_its_config_file() {
    assert_eq!(PresetFile::Local.config_file(), ConfigFile::Local);
    assert_eq!(PresetFile::Project.config_file(), ConfigFile::Project);
    assert_eq!(
        (PresetFile::Local.name(), PresetFile::Project.name()),
        ("local", "project")
    );
    assert!(serde_json::from_value::<PresetFile>(json!("team")).is_err());
}

#[test]
fn clamped_holds_every_number_in_its_range() {
    let wild = LayoutModeSettings {
        snap: 1000,
        split_step: 3.0,
        ..LayoutModeSettings::default()
    };
    let clamped = wild.clamped();
    assert_eq!((clamped.snap, clamped.split_step), (128, 0.25));
    let nan = LayoutModeSettings {
        split_step: f64::NAN,
        ..LayoutModeSettings::default()
    };
    assert_eq!(nan.clamped().split_step, 0.05);
}

#[test]
fn the_user_settings_carry_the_section() {
    let user: crate::UserSettings =
        serde_json::from_value(json!({ "layoutModes": { "snap": 4 } })).expect("reads");
    assert_eq!(user.layout_modes.snap, 4);
}

// RB-080-50.
#[test]
fn the_canvas_zooms_with_the_wheel_and_pans_with_the_middle_button_unless_switched_off() {
    let settings = LayoutModeSettings::default();
    assert!(settings.wheel_zoom);
    assert!(settings.middle_button_pan);
    let off: LayoutModeSettings =
        serde_json::from_value(json!({ "wheelZoom": false, "middleButtonPan": false }))
            .expect("reads");
    assert!(!off.wheel_zoom);
    assert!(!off.middle_button_pan);
    assert!(off.space_pan, "the other keys keep their defaults");
}

// RB-080-55.
#[test]
fn a_canvas_view_past_the_edge_shows_its_part_unless_switched_off() {
    assert!(LayoutModeSettings::default().show_partial_views);
    let off: LayoutModeSettings =
        serde_json::from_value(json!({ "showPartialViews": false })).expect("reads");
    assert!(!off.show_partial_views);
    assert!(off.wheel_zoom, "the other keys keep their defaults");
}
