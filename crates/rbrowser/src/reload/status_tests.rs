use serde_json::json;

use super::*;

#[test]
fn the_status_serialises_with_its_state() {
    let polling = WatcherStatus::Polling {
        folders: 12,
        reason_key: Some("wsl.error.helperStart".into()),
    };
    assert_eq!(
        serde_json::to_value(&polling).expect("serialises"),
        json!({ "state": "polling", "folders": 12, "reasonKey": "wsl.error.helperStart" })
    );
    assert_eq!(
        serde_json::to_value(WatcherStatus::Off).expect("serialises"),
        json!({ "state": "off" })
    );
}

#[test]
fn the_label_follows_the_state_and_a_pause_wins() {
    assert_eq!(
        WatcherStatus::Native { folders: 1 }.label_key(),
        "reload.status.watching"
    );
    let info = WatcherInfo::new(
        WatcherStatus::Native { folders: 1 },
        true,
        HotMode::Auto,
        None,
    );
    assert_eq!(info.label_key, "reload.status.paused");
    let info = WatcherInfo::new(WatcherStatus::Off, false, HotMode::Off, Some(10));
    assert_eq!(
        serde_json::to_value(&info).expect("serialises"),
        json!({
            "status": { "state": "off" },
            "labelKey": "reload.status.off",
            "paused": false,
            "mode": "off",
            "intervalSec": 10,
        })
    );
}
