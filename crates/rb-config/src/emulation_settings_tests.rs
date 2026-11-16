use serde_json::json;

use super::*;

#[test]
fn the_defaults_are_berlin_new_york_and_factor_4() {
    let settings: EmulationUserSettings = serde_json::from_value(json!({})).expect("defaults");
    assert_eq!(settings, EmulationUserSettings::default());
    assert_eq!(settings.default_geolocation, "berlin");
    assert_eq!(settings.default_timezone, "America/New_York");
    assert_eq!(settings.default_cpu_throttling, 4.0);
    assert!(crate::is_iana_timezone(&settings.default_timezone));
}

#[test]
fn four_presets_are_built_in() {
    let ids: Vec<String> = geolocation_presets(&EmulationUserSettings::default())
        .into_iter()
        .map(|preset| preset.id)
        .collect();
    assert_eq!(ids, ["berlin", "new-york", "tokyo", "sydney"]);
    let berlin = geolocation_preset(&EmulationUserSettings::default(), "berlin").expect("built in");
    assert_eq!((berlin.latitude, berlin.longitude), (52.52, 13.405));
    assert!(
        geolocation_presets(&EmulationUserSettings::default())
            .iter()
            .all(|preset| preset.position.problem().is_none())
    );
}

#[test]
fn own_presets_follow_and_replace_a_built_in_id() {
    let settings: EmulationUserSettings = serde_json::from_value(json!({
        "geolocationPresets": [
            { "id": "office", "label": "Office", "latitude": 48.1, "longitude": 11.6 },
            { "id": "tokyo", "latitude": 35.0, "longitude": 139.0, "accuracy": 10 },
        ],
    }))
    .expect("settings");
    let ids: Vec<String> = geolocation_presets(&settings)
        .into_iter()
        .map(|preset| preset.id)
        .collect();
    assert_eq!(ids, ["berlin", "new-york", "sydney", "office", "tokyo"]);
    let tokyo = geolocation_preset(&settings, "tokyo").expect("own");
    assert_eq!((tokyo.latitude, tokyo.accuracy), (35.0, 10.0));
    assert_eq!(geolocation_preset(&settings, "nowhere"), None);
}

#[test]
fn a_preset_writes_its_position_flat() {
    let preset = GeolocationPreset {
        id: "office".into(),
        label: Some("Office".into()),
        position: Geolocation {
            latitude: 1.0,
            longitude: 2.0,
            accuracy: 3.0,
        },
    };
    assert_eq!(
        serde_json::to_value(&preset).expect("serializes"),
        json!({ "id": "office", "label": "Office", "latitude": 1.0, "longitude": 2.0, "accuracy": 3.0 })
    );
}
