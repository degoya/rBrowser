use serde_json::json;

use super::*;

#[test]
fn the_state_is_left_out_of_the_snapshot_while_it_tells_nothing() {
    let state = ActivityBarState::default();
    assert!(state.is_empty());
    assert_eq!(serde_json::to_value(&state).expect("json"), json!({}));
    let state = ActivityBarState {
        place: Some(ActivityBarPosition::Bottom),
        collapsed: vec![Activity::Video, Activity::Desync],
        recorded: Some("login".into()),
    };
    assert!(!state.is_empty());
    assert_eq!(
        serde_json::to_value(&state).expect("json"),
        json!({ "place": "bottom", "collapsed": ["video", "desync"], "recorded": "login" })
    );
    let offer = ActivityBarState {
        recorded: Some("login".into()),
        ..ActivityBarState::default()
    };
    assert!(!offer.is_empty(), "an offer to play is something to tell");
}

#[test]
fn an_entry_is_folded_once_and_opened_again() {
    let mut state = ActivityBarState::default();
    assert!(state.set_collapsed(Activity::Portal, true));
    assert!(!state.set_collapsed(Activity::Portal, true), "folded once");
    assert!(state.set_collapsed(Activity::Video, true));
    assert_eq!(
        state.collapsed,
        [Activity::Video, Activity::Portal],
        "sorted"
    );
    assert!(state.set_collapsed(Activity::Portal, false));
    assert!(
        !state.set_collapsed(Activity::Portal, false),
        "already open"
    );
    assert_eq!(state.collapsed, [Activity::Video]);
}

#[test]
fn a_video_names_its_views_its_start_and_its_limit() {
    let video = VideoActivity {
        views: vec![ViewId(1), ViewId(3)],
        started_at_ms: 1_763_380_800_000,
        max_duration_sec: 600,
        paused_at_ms: None,
        paused_ms: 0,
    };
    assert_eq!(
        serde_json::to_value(&video).expect("json"),
        json!({ "views": [1, 3], "startedAtMs": 1_763_380_800_000_u64, "maxDurationSec": 600 })
    );
}

/// RB-090-68: a paused video names when it paused and the time paused before.
#[test]
fn a_paused_video_names_its_pause() {
    let video = VideoActivity {
        views: vec![ViewId(1)],
        started_at_ms: 1_763_380_800_000,
        max_duration_sec: 600,
        paused_at_ms: Some(1_763_380_830_000),
        paused_ms: 5_000,
    };
    let value = serde_json::to_value(&video).expect("json");
    assert_eq!(value["pausedAtMs"], 1_763_380_830_000_u64);
    assert_eq!(value["pausedMs"], 5_000);
    let back: VideoActivity =
        serde_json::from_value(json!({ "views": [1], "startedAtMs": 1, "maxDurationSec": 2 }))
            .expect("an older state reads");
    assert_eq!((back.paused_at_ms, back.paused_ms), (None, 0));
}
