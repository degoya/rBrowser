use rb_config::{Breakpoint, DevicePreset, EmulationFields, Geolocation, NetworkConditions};
use serde_json::{Value, json};

use super::*;
use crate::{Emulation, EngineError, resolve_emulation};

fn breakpoint(json: Value) -> Breakpoint {
    serde_json::from_value(json).expect("a breakpoint")
}

fn fields(json: Value) -> EmulationFields {
    serde_json::from_value(json).expect("emulation fields")
}

fn resolve(own: Value, defaults: Value) -> Result<Emulation, EngineError> {
    let mut own = own;
    if let Some(object) = own.as_object_mut() {
        object.insert("id".into(), json!("b"));
        object.entry("width").or_insert(json!(800));
        object.entry("height").or_insert(json!(600));
    }
    resolve_emulation(&breakpoint(own), &fields(defaults), &[])
}

fn berlin() -> Geolocation {
    Geolocation {
        latitude: 52.52,
        longitude: 13.4,
        accuracy: 50.0,
    }
}

#[test]
fn the_extras_resolve_own_over_defaults() {
    let resolved = resolve(
        json!({ "timezone": "America/New_York", "cpuThrottling": 4 }),
        json!({
            "geolocation": { "latitude": 52.52, "longitude": 13.4 },
            "timezone": "Europe/Berlin",
            "cpuThrottling": 2,
        }),
    )
    .expect("resolves");
    assert_eq!(resolved.geolocation, Some(berlin()));
    assert_eq!(resolved.timezone.as_deref(), Some("America/New_York"));
    assert_eq!(resolved.cpu_throttling, Some(4.0));
}

#[test]
fn null_empty_and_1_switch_a_default_off() {
    let resolved = resolve(
        json!({ "geolocation": null, "timezone": "", "cpuThrottling": 1 }),
        json!({
            "geolocation": { "latitude": 52.52, "longitude": 13.4 },
            "timezone": "Europe/Berlin",
            "cpuThrottling": 4,
        }),
    )
    .expect("resolves");
    assert_eq!(resolved.geolocation, None);
    assert_eq!(resolved.timezone, None);
    assert_eq!(resolved.cpu_throttling, None, "1 is no throttling");
}

#[test]
fn bad_extras_are_invalid() {
    for own in [
        json!({ "geolocation": { "latitude": 91, "longitude": 0 } }),
        json!({ "timezone": "Mars/Olympus_Mons" }),
        json!({ "cpuThrottling": 0.5 }),
        json!({ "cpuThrottling": 101 }),
    ] {
        let result = resolve(own.clone(), json!({}));
        assert!(matches!(result, Err(EngineError::Invalid(_))), "{own}");
    }
}

#[test]
fn offline_goes_through_the_network_conditions() {
    // (own, defaults, expected conditions)
    let slow = NetworkConditions {
        offline: false,
        latency_ms: 2000,
        download_kbps: 400,
        upload_kbps: 400,
    };
    let cases = [
        (
            json!({ "offline": true }),
            json!({}),
            Some(NetworkConditions {
                offline: true,
                ..NetworkConditions::default()
            }),
        ),
        (json!({ "offline": false }), json!({}), None),
        (
            json!({ "offline": true, "network": "slow-3g" }),
            json!({}),
            Some(NetworkConditions {
                offline: true,
                ..slow
            }),
        ),
        (
            json!({ "offline": false }),
            json!({ "network": "offline" }),
            Some(NetworkConditions::default()),
        ),
        (
            json!({}),
            json!({ "offline": true }),
            Some(NetworkConditions {
                offline: true,
                ..NetworkConditions::default()
            }),
        ),
        (json!({ "network": "slow-3g" }), json!({}), Some(slow)),
    ];
    for (own, defaults, expected) in cases {
        let resolved = resolve(own.clone(), defaults.clone()).expect("resolves");
        assert_eq!(resolved.network, expected, "{own} over {defaults}");
    }
}

#[test]
fn a_print_breakpoint_is_as_wide_as_its_paper() {
    let resolved = resolve_emulation(
        &breakpoint(json!({
            "id": "print-a4", "paper": "A4", "media": { "mediaType": "print" }
        })),
        &EmulationFields::default(),
        &[],
    )
    .expect("resolves");
    assert_eq!((resolved.width, resolved.height), (794, 1123));
    assert_eq!(resolved.media.media_type, Some(rb_config::MediaType::Print));
}

#[test]
fn the_paper_comes_before_the_device_but_after_own_sides() {
    let phone: DevicePreset = serde_json::from_value(json!({
        "id": "phone", "kind": "mobile", "viewport": { "width": 390, "height": 844 }, "dpr": 3.0
    }))
    .expect("a device");
    let resolved = resolve_emulation(
        &breakpoint(json!({ "id": "p", "device": "phone", "paper": "Letter", "height": 900 })),
        &EmulationFields::default(),
        &[phone],
    )
    .expect("resolves");
    assert_eq!((resolved.width, resolved.height), (816, 900));
    assert_eq!(resolved.dpr, 3.0, "the rest still comes from the device");
}

#[test]
fn the_extras_serialize_camel_case_and_only_when_set() {
    let emulation = Emulation {
        geolocation: Some(berlin()),
        timezone: Some("Asia/Tokyo".into()),
        cpu_throttling: Some(4.0),
        ..Emulation::default()
    };
    let json = serde_json::to_value(&emulation).expect("serializes");
    assert_eq!(json["geolocation"]["latitude"], 52.52);
    assert_eq!(json["timezone"], "Asia/Tokyo");
    assert_eq!(json["cpuThrottling"], 4.0);
    let plain = serde_json::to_value(Emulation::default()).expect("serializes");
    for field in ["geolocation", "timezone", "cpuThrottling"] {
        assert!(plain.get(field).is_none(), "{field}");
    }
}

#[test]
fn support_per_engine_and_field() {
    use EngineKind::{Chromium, Firefox, Webkit};
    let cases = [
        (Chromium, GEOLOCATION, Support::Yes),
        (Chromium, TIMEZONE, Support::Yes),
        (Chromium, CPU_THROTTLING, Support::Yes),
        (Chromium, OFFLINE, Support::Yes),
        (Chromium, PRINT, Support::Yes),
        (
            Firefox,
            GEOLOCATION,
            Support::Probe("emulation.setGeolocationOverride"),
        ),
        (
            Firefox,
            TIMEZONE,
            Support::Probe("emulation.setTimezoneOverride"),
        ),
        (Firefox, CPU_THROTTLING, Support::No),
        (
            Firefox,
            OFFLINE,
            Support::Probe("emulation.setNetworkConditions"),
        ),
        (Firefox, PRINT, Support::No),
        (Webkit, GEOLOCATION, Support::Yes),
        (Webkit, TIMEZONE, Support::NewContext),
        (Webkit, CPU_THROTTLING, Support::No),
        (Webkit, OFFLINE, Support::Yes),
        (Webkit, PRINT, Support::Yes),
    ];
    for (engine, field, expected) in cases {
        assert_eq!(support(engine, field), expected, "{engine:?} {field}");
    }
}

#[test]
fn not_emulable_lists_the_set_fields_an_engine_cannot_do() {
    let mut everything = Emulation {
        geolocation: Some(berlin()),
        timezone: Some("Asia/Tokyo".into()),
        cpu_throttling: Some(4.0),
        network: Some(NetworkConditions {
            offline: true,
            ..NetworkConditions::default()
        }),
        ..Emulation::default()
    };
    everything.media.media_type = Some(rb_config::MediaType::Print);
    assert!(not_emulable(EngineKind::Chromium, &everything, &[]).is_empty());
    assert_eq!(
        not_emulable(EngineKind::Webkit, &everything, &[]),
        [CPU_THROTTLING]
    );
    assert_eq!(
        not_emulable(EngineKind::Firefox, &everything, &[]),
        [GEOLOCATION, TIMEZONE, CPU_THROTTLING, OFFLINE, PRINT]
    );
    assert_eq!(
        not_emulable(
            EngineKind::Firefox,
            &everything,
            &[
                "emulation.setGeolocationOverride",
                "emulation.setTimezoneOverride"
            ],
        ),
        [CPU_THROTTLING, OFFLINE, PRINT]
    );
    assert!(not_emulable(EngineKind::Firefox, &Emulation::default(), &[]).is_empty());
}
