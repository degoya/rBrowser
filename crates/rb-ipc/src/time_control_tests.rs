use serde_json::json;

use super::*;

#[test]
fn a_fixed_clock_and_stopped_animations_are_written_in_camel_case() {
    let activity = TimeControlActivity {
        clock: Some(ClockActivity {
            start: "2026-12-24T10:00:00".into(),
            local: true,
            speed: 0.0,
        }),
        animations: vec![
            ViewRate {
                view: ViewId(1),
                rate: 0.0,
            },
            ViewRate {
                view: ViewId(3),
                rate: 0.25,
            },
        ],
    };
    assert_eq!(
        serde_json::to_value(&activity).expect("json"),
        json!({
            "clock": { "start": "2026-12-24T10:00:00", "local": true, "speed": 0.0 },
            "animations": [{ "view": 1, "rate": 0.0 }, { "view": 3, "rate": 0.25 }],
        })
    );
    let back: TimeControlActivity =
        serde_json::from_value(serde_json::to_value(&activity).expect("json")).expect("back");
    assert_eq!(back, activity);
}

#[test]
fn what_is_unchanged_is_left_out() {
    let clock_only = TimeControlActivity {
        clock: Some(ClockActivity {
            start: "2026-12-24T09:00:00.000Z".into(),
            local: false,
            speed: 1.0,
        }),
        animations: Vec::new(),
    };
    assert_eq!(
        serde_json::to_value(&clock_only).expect("json"),
        json!({ "clock": { "start": "2026-12-24T09:00:00.000Z", "local": false, "speed": 1.0 } })
    );
    assert!(!clock_only.is_empty());
    assert!(TimeControlActivity::default().is_empty());
}
