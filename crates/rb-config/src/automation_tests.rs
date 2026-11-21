use serde_json::{Value, json};

use super::*;

#[test]
fn the_recorder_defaults_keep_hovers_and_scrolls() {
    // RB-090-66: scrolling is recorded by default, so a played recording scrolls as it did.
    let config: AutomationConfig = serde_json::from_value(json!({})).expect("defaults");
    assert!(config.recorder.record_scroll);
    // RB-090-81: only the hover the agent marked as a menu's is a step by default.
    assert_eq!(config.recorder.record_hover, RecordHover::Menu);
    // RB-090-78: the form autofill's fills are recorded by default.
    assert!(config.recorder.record_autofill);
    assert_eq!(config.recorder.playback_pace, PlaybackPace::Fast);
    // RB-090-84: the panel follows a played recording by default.
    assert!(config.recorder.follow_playback);
    assert_eq!(config.recorder.min_pause_ms, 0);
    assert_eq!(config.recorder.dir, ".rbrowser/out/recordings");
    assert!(config.recorder.secret_fields.is_empty());
}

#[test]
fn the_playback_pace_is_fast_or_recorded() {
    let config: AutomationConfig =
        serde_json::from_value(json!({ "recorder": { "playbackPace": "recorded" } }))
            .expect("parses");
    assert_eq!(config.recorder.playback_pace, PlaybackPace::Recorded);
    assert_eq!(PlaybackPace::Recorded.as_str(), "recorded");
    assert_eq!(PlaybackPace::Fast.as_str(), "fast");
    assert!(
        serde_json::from_value::<AutomationConfig>(
            json!({ "recorder": { "playbackPace": "slow" } })
        )
        .is_err()
    );
}

#[test]
fn the_recorder_keys_are_camel_case() {
    let config: AutomationConfig = serde_json::from_value(json!({
        "recorder": { "recordScroll": false, "recordAutofill": false, "secretFields": ["#otp"],
                      "dir": ".rbrowser/rec" }
    }))
    .expect("parses");
    assert!(!config.recorder.record_scroll);
    assert_eq!(config.recorder.record_hover, RecordHover::Menu);
    assert!(!config.recorder.record_autofill);
    assert_eq!(config.recorder.secret_fields, ["#otp"]);
    assert_eq!(config.recorder.dir, ".rbrowser/rec");
}

#[test]
fn record_hover_is_menu_all_or_off() {
    for (value, expected) in [
        ("menu", RecordHover::Menu),
        ("all", RecordHover::All),
        ("off", RecordHover::Off),
    ] {
        let config: AutomationConfig =
            serde_json::from_value(json!({ "recorder": { "recordHover": value } }))
                .expect("parses");
        assert_eq!(config.recorder.record_hover, expected);
        assert_eq!(expected.as_str(), value);
        let written = serde_json::to_value(&config.recorder).expect("serialises");
        assert_eq!(written["recordHover"], value);
    }
    assert!(
        serde_json::from_value::<AutomationConfig>(
            json!({ "recorder": { "recordHover": "some" } })
        )
        .is_err()
    );
}

#[test]
fn the_old_record_hover_switch_still_loads() {
    // RB-090-81: `true` kept the menu hovers, `false` none.
    for (old, now) in [(true, RecordHover::Menu), (false, RecordHover::Off)] {
        let config: AutomationConfig =
            serde_json::from_value(json!({ "recorder": { "recordHover": old } })).expect("parses");
        assert_eq!(config.recorder.record_hover, now);
    }
}

#[test]
fn the_record_hover_schema_offers_the_three_choices() {
    let schema = serde_json::to_value(schemars::schema_for!(RecorderConfig)).expect("schema");
    let text = schema.to_string();
    for choice in ["\"menu\"", "\"all\"", "\"off\""] {
        assert!(text.contains(choice), "{choice} in {text}");
    }
    assert_eq!(schema["properties"]["recordHover"]["default"], "menu");
    // The earlier switch still validates in an editor, marked as the earlier form; the settings
    // window offers only the three choices (RB-090-81).
    let branches = schema["properties"]["recordHover"]["oneOf"]
        .as_array()
        .expect("a oneOf");
    let consts: Vec<&Value> = branches.iter().filter_map(|b| b.get("const")).collect();
    assert_eq!(consts, [&json!("menu"), &json!("all"), &json!("off")]);
    let earlier: Vec<&Value> = branches.iter().filter(|b| b["type"] == "boolean").collect();
    assert_eq!(earlier.len(), 1, "{branches:?}");
    assert_eq!(earlier[0]["deprecated"], true);
}

#[test]
fn the_export_defaults_write_the_config_into_out_exports() {
    let config: AutomationConfig = serde_json::from_value(json!({})).expect("defaults");
    let export = &config.recorder_export;
    assert_eq!(export.locator_preference, LocatorPreference::Auto);
    assert_eq!(export.dir, ".rbrowser/out/exports");
    assert!(export.include_config);
}

#[test]
fn the_export_keys_are_camel_case() {
    let config: AutomationConfig = serde_json::from_value(json!({
        "recorderExport": { "locatorPreference": "css", "includeConfig": false }
    }))
    .expect("parses");
    assert_eq!(
        config.recorder_export.locator_preference,
        LocatorPreference::Css
    );
    assert_eq!(config.recorder_export.locator_preference.as_str(), "css");
    assert!(!config.recorder_export.include_config);
    assert!(
        serde_json::from_value::<AutomationConfig>(
            json!({ "recorderExport": { "locatorPreference": "xpath" } })
        )
        .is_err()
    );
}

#[test]
fn following_the_playback_can_be_switched_off() {
    let config: AutomationConfig = serde_json::from_value(
        json!({ "recorder": { "followPlayback": false, "minPauseMs": 1500 } }),
    )
    .expect("parses");
    assert!(!config.recorder.follow_playback);
    assert_eq!(config.recorder.min_pause_ms, 1_500);
}

#[test]
fn the_live_steps_show_by_default_and_can_be_switched_off() {
    // RB-090-89: the Recorder panel shows each step while it is recorded.
    let config: AutomationConfig = serde_json::from_value(json!({})).expect("defaults");
    assert!(config.recorder.show_live_steps);
    let config: AutomationConfig =
        serde_json::from_value(json!({ "recorder": { "showLiveSteps": false } })).expect("parses");
    assert!(!config.recorder.show_live_steps);
}

#[test]
fn deleting_a_recording_asks_first_by_default_and_can_be_switched_off() {
    // RB-090-91: "Delete" in the Recorder panel asks in the panel before the file goes.
    let config: AutomationConfig = serde_json::from_value(json!({})).expect("defaults");
    assert!(config.recorder.confirm_delete);
    let config: AutomationConfig =
        serde_json::from_value(json!({ "recorder": { "confirmDelete": false } })).expect("parses");
    assert!(!config.recorder.confirm_delete);
}

#[test]
fn a_menu_hover_rests_250_ms_and_the_pointer_path_follows_menus_by_default() {
    let config: AutomationConfig = serde_json::from_value(json!({})).expect("defaults");
    assert_eq!(config.recorder.hover_dwell_ms, 250);
    assert_eq!(config.recorder.record_pointer, RecordPointer::Menu);
}

#[test]
fn the_dwell_and_the_pointer_path_are_set_in_camel_case() {
    for (value, expected) in [
        ("menu", RecordPointer::Menu),
        ("all", RecordPointer::All),
        ("off", RecordPointer::Off),
    ] {
        let config: AutomationConfig = serde_json::from_value(
            json!({ "recorder": { "hoverDwellMs": 400, "recordPointer": value } }),
        )
        .expect("parses");
        assert_eq!(config.recorder.hover_dwell_ms, 400);
        assert_eq!(config.recorder.record_pointer, expected);
        let written = serde_json::to_value(&config.recorder).expect("writes");
        assert_eq!(written["recordPointer"], value);
        assert_eq!(written["hoverDwellMs"], 400);
    }
    assert!(
        serde_json::from_value::<AutomationConfig>(
            json!({ "recorder": { "recordPointer": "some" } })
        )
        .is_err()
    );
}

#[test]
fn the_dwell_schema_is_bounded() {
    let schema = serde_json::to_value(schemars::schema_for!(RecorderConfig)).expect("schema");
    assert_eq!(schema["properties"]["hoverDwellMs"]["default"], 250);
    assert_eq!(
        schema["properties"]["hoverDwellMs"]["maximum"].as_f64(),
        Some(5_000.0)
    );
    assert_eq!(schema["properties"]["recordPointer"]["default"], "menu");
}

#[test]
fn a_step_added_in_the_editor_waits_half_a_second_by_default() {
    // RB-092-01: the pause before a step added in the Recorder panel's editor.
    let config: AutomationConfig = serde_json::from_value(json!({})).expect("defaults");
    assert_eq!(config.recorder.inserted_step_pause_ms, 500);
    let config: AutomationConfig =
        serde_json::from_value(json!({ "recorder": { "insertedStepPauseMs": 0 } }))
            .expect("parses");
    assert_eq!(config.recorder.inserted_step_pause_ms, 0);
}
