use serde_json::json;

use super::*;
use crate::UserSettings;

#[test]
fn the_status_bar_defaults_and_reads_its_section() {
    let user: UserSettings = serde_json::from_value(json!({})).expect("settings");
    assert_eq!(user.ui.status_bar, StatusBarSettings::default());
    assert!(user.ui.status_bar.visible);
    assert_eq!(user.ui.status_bar.info_timeout_sec, 6);
    assert_eq!(user.ui.status_bar.history_size, 200);
    let user: UserSettings = serde_json::from_value(json!({
        "ui": { "theme": "dark", "statusBar": { "visible": false, "historySize": 50 } }
    }))
    .expect("settings");
    assert!(!user.ui.status_bar.visible);
    assert_eq!(user.ui.status_bar.history_size, 50);
    assert_eq!(user.ui.status_bar.info_timeout_sec, 6);
}

#[test]
fn the_history_size_is_held_to_its_range() {
    let size = |history_size| {
        StatusBarSettings {
            history_size,
            ..StatusBarSettings::default()
        }
        .history_capacity()
    };
    assert_eq!(size(0), MIN_HISTORY_SIZE as usize);
    assert_eq!(size(50), 50);
    assert_eq!(size(u32::MAX), MAX_HISTORY_SIZE as usize);
}

#[test]
fn the_schema_says_the_ranges() {
    let schema = serde_json::to_value(schemars::schema_for!(StatusBarSettings)).expect("schema");
    let props = &schema["properties"];
    assert_eq!(props["historySize"]["minimum"], json!(MIN_HISTORY_SIZE));
    assert_eq!(props["historySize"]["maximum"], json!(MAX_HISTORY_SIZE));
    assert_eq!(
        props["infoTimeoutSec"]["maximum"],
        json!(MAX_INFO_TIMEOUT_SEC)
    );
}

#[test]
fn the_canvas_zoom_shows_unless_switched_off() {
    let user: crate::UserSettings =
        serde_json::from_value(serde_json::json!({})).expect("settings");
    assert!(user.ui.status_bar.canvas_zoom);
    let off: crate::UserSettings = serde_json::from_value(serde_json::json!({
        "ui": { "statusBar": { "canvasZoom": false } }
    }))
    .expect("settings");
    assert!(!off.ui.status_bar.canvas_zoom);
    assert!(off.ui.status_bar.visible, "the bar itself stays");
}
