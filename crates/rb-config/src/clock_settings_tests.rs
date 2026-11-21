use serde_json::json;

use super::*;
use crate::EmulationUserSettings;

#[test]
fn the_defaults_freeze_the_moment_and_offer_stop_10_25_and_100_percent() {
    let settings: EmulationUserSettings = serde_json::from_value(json!({})).expect("defaults");
    assert_eq!(settings.clock, ClockUserSettings::default());
    assert_eq!(settings.clock.default, "");
    assert_eq!(settings.clock.speed, 0.0);
    assert_eq!(settings.animation_rates, DEFAULT_ANIMATION_RATES.to_vec());
    assert_eq!(settings.animation_rates, [0.0, 0.1, 0.25, 1.0]);
    assert_eq!(settings.animation_step_ms, DEFAULT_ANIMATION_STEP_MS);
    assert_eq!(
        serde_json::to_value(&settings).expect("json")["clock"],
        json!({ "default": "", "speed": 0.0 })
    );
}

#[test]
fn a_file_sets_the_clock_the_rates_and_the_step() {
    let settings: EmulationUserSettings = serde_json::from_value(json!({
        "clock": { "default": "2026-12-24T10:00", "speed": 2 },
        "animationRates": [0, 0.5, 1],
        "animationStepMs": 40,
    }))
    .expect("settings");
    assert_eq!(settings.clock.default, "2026-12-24T10:00");
    assert_eq!(settings.clock.speed, 2.0);
    assert_eq!(settings.animation_rates, [0.0, 0.5, 1.0]);
    assert_eq!(settings.animation_step_ms, 40);
    // A key the file leaves out keeps its default.
    let only_speed: EmulationUserSettings =
        serde_json::from_value(json!({ "clock": { "speed": 1 } })).expect("settings");
    assert_eq!(only_speed.clock.default, "");
}

#[test]
fn the_offered_rates_are_the_settings_choices() {
    for rate in DEFAULT_ANIMATION_RATES {
        assert!(ANIMATION_RATE_CHOICES.contains(&rate), "{rate}");
    }
    assert!(ANIMATION_RATE_CHOICES.contains(&1.0));
    assert!(
        ANIMATION_RATE_CHOICES
            .windows(2)
            .all(|pair| pair[0] < pair[1])
    );
}
