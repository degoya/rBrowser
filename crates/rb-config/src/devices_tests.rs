use std::collections::HashSet;

use super::*;
use crate::{DeviceKind, Viewport};

fn device(id: &str, width: u32) -> DevicePreset {
    DevicePreset {
        id: id.to_owned(),
        label: Some(format!("My {id}")),
        kind: DeviceKind::Mobile,
        viewport: Viewport { width, height: 800 },
        dpr: 2.0,
        ua: None,
        platform: None,
        touch: true,
        orientation: None,
        safe_areas: None,
    }
}

fn ids(devices: &[DevicePreset]) -> Vec<&str> {
    devices.iter().map(|device| device.id.as_str()).collect()
}

#[test]
fn the_builtin_list_loads_with_unique_valid_presets_of_every_kind() {
    let devices = builtin_devices().expect("the built-in list parses");
    assert!(devices.len() >= 12, "{} devices", devices.len());
    let unique: HashSet<&str> = ids(&devices).into_iter().collect();
    assert_eq!(unique.len(), devices.len(), "duplicate ids");
    for kind in [DeviceKind::Mobile, DeviceKind::Tablet, DeviceKind::Desktop] {
        assert!(devices.iter().any(|device| device.kind == kind), "{kind:?}");
    }
    for device in &devices {
        assert!(
            device.label.is_none(),
            "{}: built-ins are named by i18n keys",
            device.id
        );
        assert!(check_device(device, "").is_empty(), "{}", device.id);
    }
}

#[test]
fn iphone_15_carries_viewport_dpr_ua_and_touch() {
    let devices = builtin_devices().expect("parses");
    let iphone = devices
        .iter()
        .find(|device| device.id == "iphone-15")
        .expect("iphone-15 is built in");
    assert_eq!(
        iphone.viewport,
        Viewport {
            width: 393,
            height: 852
        }
    );
    assert!((iphone.dpr - 3.0).abs() < f64::EPSILON);
    assert!(iphone.touch);
    assert!(iphone.ua.as_deref().is_some_and(|ua| ua.contains("iPhone")));
    assert!(iphone.safe_areas.is_some());
}

#[test]
fn user_presets_are_added_and_replace_a_builtin_of_the_same_id_in_place() {
    let builtin = builtin_devices().expect("parses");
    let user = DeviceSettings {
        replace_defaults: false,
        items: vec![device("iphone-15", 400), device("my-phone", 360)],
    };
    let effective = effective_devices(&user).expect("merges");
    assert_eq!(effective.len(), builtin.len() + 1);
    let position = |devices: &[DevicePreset], id: &str| {
        devices
            .iter()
            .position(|device| device.id == id)
            .expect("present")
    };
    assert_eq!(
        position(&effective, "iphone-15"),
        position(&builtin, "iphone-15")
    );
    assert_eq!(
        effective[position(&effective, "iphone-15")].viewport.width,
        400
    );
    assert_eq!(
        effective.last().map(|device| device.id.as_str()),
        Some("my-phone")
    );
}

#[test]
fn replace_defaults_keeps_only_the_users_presets() {
    let user = DeviceSettings {
        replace_defaults: true,
        items: vec![device("a", 360), device("b", 390)],
    };
    let effective = effective_devices(&user).expect("merges");
    assert_eq!(ids(&effective), ["a", "b"]);
}

#[test]
fn no_user_presets_is_the_builtin_list() {
    let effective = effective_devices(&DeviceSettings::default()).expect("merges");
    assert_eq!(effective, builtin_devices().expect("parses"));
}

#[test]
fn a_later_user_preset_with_the_same_id_wins() {
    let user = DeviceSettings {
        replace_defaults: true,
        items: vec![device("a", 360), device("a", 412)],
    };
    let effective = effective_devices(&user).expect("merges");
    assert_eq!(ids(&effective), ["a"]);
    assert_eq!(effective[0].viewport.width, 412);
}

#[test]
fn an_invalid_user_preset_is_refused_with_its_path() {
    let mut bad = device("", 0);
    bad.dpr = f64::NAN;
    let user = DeviceSettings {
        replace_defaults: false,
        items: vec![device("ok", 360), bad],
    };
    let Err(ConfigError::Invalid(issues)) = effective_devices(&user) else {
        panic!("an empty id, a zero width and a NaN DPR are refused");
    };
    let paths: Vec<&str> = issues.iter().map(|issue| issue.path.as_str()).collect();
    assert_eq!(
        paths,
        [
            "/devices/items/1/id",
            "/devices/items/1/viewport",
            "/devices/items/1/dpr"
        ]
    );
    assert!(
        issues
            .iter()
            .all(|issue| issue.key.starts_with("config.error.device_"))
    );
}
