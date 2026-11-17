use serde_json::json;

use super::*;
use crate::UserSettings;

#[test]
fn the_bar_stands_below_the_toolbar_and_shows_every_activity_by_default() {
    let user: UserSettings = serde_json::from_value(json!({})).expect("settings");
    let bar = &user.ui.activity_bar;
    assert_eq!(bar.position, ActivityBarPosition::Top);
    assert_eq!(bar.mode, ActivityBarMode::Bar);
    assert_eq!(bar.show, ActivityBarShow::default());
    assert!(bar.show.video && bar.show.recorder && bar.show.flows);
    assert!(bar.show.checks && bar.show.portal && bar.show.desync);
    assert_eq!(
        serde_json::to_value(bar).expect("json"),
        json!({
            "position": "top",
            "mode": "bar",
            "show": {
                "video": true,
                "recorder": true,
                "flows": true,
                "checks": true,
                "portal": true,
                "desync": true
            }
        })
    );
}

#[test]
fn a_file_moves_the_bar_down_shows_only_indicators_or_leaves_an_activity_out() {
    let user: UserSettings = serde_json::from_value(json!({
        "ui": { "activityBar": {
            "position": "bottom",
            "mode": "indicator",
            "show": { "portal": false }
        } }
    }))
    .expect("settings");
    let bar = &user.ui.activity_bar;
    assert_eq!(bar.position, ActivityBarPosition::Bottom);
    assert_eq!(bar.mode, ActivityBarMode::Indicator);
    assert!(!bar.show.portal);
    // A key the file leaves out keeps its default.
    assert!(bar.show.video && bar.show.desync);
}

#[test]
fn an_unknown_position_or_mode_is_refused() {
    for bad in [
        json!({ "ui": { "activityBar": { "position": "left" } } }),
        json!({ "ui": { "activityBar": { "mode": "hidden" } } }),
    ] {
        assert!(
            serde_json::from_value::<UserSettings>(bad.clone()).is_err(),
            "{bad}"
        );
    }
}
