use super::*;
use crate::{Breakpoint, EngineKind, UserSettings};

#[test]
fn ios_takes_the_simulator_when_xcode_is_there_by_default() {
    let settings: UserSettings = serde_json::from_str(r#"{ "version": 1 }"#).expect("parses");
    assert_eq!(
        settings.engines.ios,
        IosSettings {
            mode: "auto".to_owned(),
            default_simulator: "iPhone 16".to_owned(),
            boot_timeout_sec: 120,
        }
    );
    let json = r#"{ "version": 1, "engines": { "ios": { "mode": "device", "defaultSimulator": "iPad Air 11-inch (M2)", "bootTimeoutSec": 45 } } }"#;
    let settings: UserSettings = serde_json::from_str(json).expect("parses");
    assert_eq!(settings.engines.ios.mode, "device");
    assert_eq!(
        settings.engines.ios.default_simulator,
        "iPad Air 11-inch (M2)"
    );
    assert_eq!(settings.engines.ios.boot_timeout_sec, 45);
}

#[test]
fn the_schema_offers_the_three_modes_and_bounds_the_boot_wait() {
    let schema = serde_json::to_value(schemars::schema_for!(IosSettings)).expect("a schema");
    let mode = &schema["properties"]["mode"];
    assert_eq!(
        mode["enum"],
        serde_json::json!(["auto", "simulator", "device"])
    );
    let boot = &schema["properties"]["bootTimeoutSec"];
    assert_eq!(boot["minimum"], 10);
    assert_eq!(boot["maximum"], 600);
}

#[test]
fn ios_is_an_engine_of_its_own_in_the_config() {
    assert_eq!(EngineKind::Ios.name(), "ios");
    let parsed: EngineKind = serde_json::from_str(r#""ios""#).expect("parses");
    assert_eq!(parsed, EngineKind::Ios);
    assert_ne!(EngineKind::Ios, EngineKind::Safari);
}

#[test]
fn a_breakpoint_names_its_ios_device_and_leaves_it_out_when_absent() {
    let json = r#"{ "id": "phone", "engine": "ios", "iosDevice": "iPhone 16" }"#;
    let breakpoint: Breakpoint = serde_json::from_str(json).expect("parses");
    assert_eq!(breakpoint.engine, EngineKind::Ios);
    assert_eq!(breakpoint.ios_device.as_deref(), Some("iPhone 16"));
    let plain: Breakpoint =
        serde_json::from_str(r#"{ "id": "desk", "width": 1280 }"#).expect("parses");
    assert_eq!(plain.ios_device, None);
    let written = serde_json::to_value(&plain).expect("serializes");
    assert!(written.get("iosDevice").is_none(), "{written}");
}
