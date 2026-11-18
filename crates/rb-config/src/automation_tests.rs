use serde_json::json;

use super::*;

#[test]
fn the_recorder_defaults_keep_hovers_and_scrolls() {
    // RB-090-66: scrolling is recorded by default, so a played recording scrolls as it did.
    let config: AutomationConfig = serde_json::from_value(json!({})).expect("defaults");
    assert!(config.recorder.record_hover && config.recorder.record_scroll);
    // RB-090-78: the form autofill's fills are recorded by default.
    assert!(config.recorder.record_autofill);
    assert_eq!(config.recorder.playback_pace, PlaybackPace::Fast);
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
    assert!(!config.recorder.record_scroll && config.recorder.record_hover);
    assert!(!config.recorder.record_autofill);
    assert_eq!(config.recorder.secret_fields, ["#otp"]);
    assert_eq!(config.recorder.dir, ".rbrowser/rec");
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
