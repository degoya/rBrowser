use serde_json::json;

use super::*;

#[test]
fn the_menu_names_each_device_with_its_state_and_whether_apple_engines_run() {
    let menu = DevicesMenu {
        android: vec![
            AndroidDeviceRow {
                serial: "R5CX".into(),
                name: "SM-S938B".into(),
                state: AndroidDeviceState::Ready,
            },
            AndroidDeviceRow {
                serial: "EMU1".into(),
                name: "EMU1".into(),
                state: AndroidDeviceState::Unauthorized,
            },
        ],
        android_reason: None,
        apple: false,
    };
    let json = serde_json::to_value(&menu).expect("json");
    assert_eq!(
        json,
        json!({
            "android": [
                { "serial": "R5CX", "name": "SM-S938B", "state": "ready" },
                { "serial": "EMU1", "name": "EMU1", "state": "unauthorized" },
            ],
            "apple": false,
        })
    );
    let back: DevicesMenu = serde_json::from_value(json).expect("reads back");
    assert_eq!(back, menu);
}

#[test]
fn an_empty_list_says_why() {
    let menu = DevicesMenu {
        android: Vec::new(),
        android_reason: Some("No adb was found.".into()),
        apple: true,
    };
    assert_eq!(
        serde_json::to_value(&menu).expect("json"),
        json!({ "android": [], "androidReason": "No adb was found.", "apple": true })
    );
    for (state, name) in [
        (AndroidDeviceState::Offline, "offline"),
        (AndroidDeviceState::NoPermissions, "noPermissions"),
        (AndroidDeviceState::Other, "other"),
    ] {
        assert_eq!(serde_json::to_value(state).expect("json"), json!(name));
    }
}
