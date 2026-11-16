use serde_json::{Value, json};

use super::*;
use crate::{
    ConfigError, DeviceKind, DeviceSettings, builtin_devices, effective_devices, validate::validate,
};

fn breakpoint(value: Value) -> Breakpoint {
    serde_json::from_value(value).expect("a breakpoint")
}

fn config(breakpoints: Value) -> ProjectConfig {
    serde_json::from_value(json!({ "version": 1, "breakpoints": breakpoints })).expect("a config")
}

fn builtin() -> Vec<DevicePreset> {
    builtin_devices().expect("the built-in list parses")
}

fn own(id: &str, width: u32, height: u32) -> DevicePreset {
    DevicePreset {
        id: id.to_owned(),
        label: Some(format!("My {id}")),
        kind: DeviceKind::Mobile,
        viewport: Viewport { width, height },
        dpr: 2.0,
        ua: None,
        platform: None,
        touch: true,
        orientation: None,
        safe_areas: None,
    }
}

#[test]
fn a_breakpoint_with_only_a_preset_takes_the_presets_viewport() {
    let phone = breakpoint(json!({ "id": "phone", "device": "iphone-15" }));
    assert_eq!(
        phone.viewport(&builtin()),
        Some(Viewport {
            width: 393,
            height: 852
        })
    );
}

#[test]
fn a_width_on_the_breakpoint_overrides_the_presets() {
    let wide = breakpoint(json!({ "id": "wide", "device": "iphone-15", "width": 430 }));
    assert_eq!(
        wide.viewport(&builtin()),
        Some(Viewport {
            width: 430,
            height: 852
        })
    );
    let both = breakpoint(json!({
        "id": "both", "device": "iphone-15", "width": 500, "height": 700
    }));
    assert_eq!(
        both.viewport(&[]),
        Some(Viewport {
            width: 500,
            height: 700
        }),
        "a full size needs no preset"
    );
}

#[test]
fn a_breakpoint_without_size_and_without_a_known_preset_has_no_viewport() {
    let bare = breakpoint(json!({ "id": "bare" }));
    assert_eq!(bare.viewport(&builtin()), None);
    let unknown = breakpoint(json!({ "id": "x", "device": "no-such-phone", "width": 400 }));
    assert_eq!(unknown.viewport(&builtin()), None);
}

#[test]
fn a_user_preset_replacing_a_default_gives_its_own_viewport() {
    let user = DeviceSettings {
        replace_defaults: false,
        items: vec![own("iphone-15", 400, 900)],
    };
    let devices = effective_devices(&user).expect("merges");
    let phone = breakpoint(json!({ "id": "phone", "device": "iphone-15" }));
    assert_eq!(
        phone.viewport(&devices),
        Some(Viewport {
            width: 400,
            height: 900
        })
    );
    let only_mine = DeviceSettings {
        replace_defaults: true,
        items: vec![own("my-phone", 360, 780)],
    };
    let devices = effective_devices(&only_mine).expect("replaces");
    let mine = breakpoint(json!({ "id": "mine", "device": "my-phone" }));
    assert_eq!(
        mine.viewport(&devices),
        Some(Viewport {
            width: 360,
            height: 780
        })
    );
    assert_eq!(phone.viewport(&devices), None, "the default is gone");
}

#[test]
fn neither_size_nor_device_is_a_validation_error_at_the_missing_fields() {
    let value = json!({
        "version": 1,
        "breakpoints": [{ "id": "bare" }, { "id": "half", "width": 800 }]
    });
    let Err(ConfigError::Invalid(issues)) = validate(value) else {
        panic!("expected issues");
    };
    let at: Vec<(&str, &str)> = issues
        .iter()
        .map(|issue| (issue.path.as_str(), issue.key.as_str()))
        .collect();
    assert_eq!(
        at,
        vec![
            ("/breakpoints/0/width", "config.error.size_required"),
            ("/breakpoints/0/height", "config.error.size_required"),
            ("/breakpoints/1/height", "config.error.size_required"),
        ]
    );
    assert!(
        issues[0].message.contains("`device`") && issues[0].message.contains("bare"),
        "{}",
        issues[0].message
    );
}

#[test]
fn a_breakpoint_naming_a_preset_passes_validation_without_a_size() {
    let value = json!({
        "version": 1,
        "breakpoints": [{ "id": "phone", "device": "iphone-15" }]
    });
    let config = validate(value).expect("valid");
    assert_eq!(config.breakpoints[0].width, None);
}

#[test]
fn a_size_taken_from_an_unknown_preset_is_reported_at_the_device() {
    let config = config(json!([
        { "id": "phone", "device": "iphone-15" },
        { "id": "ghost", "device": "no-such-phone", "height": 800 },
        { "id": "sized", "device": "no-such-phone", "width": 400, "height": 800 }
    ]));
    let issues = check_viewports(&config, &builtin());
    assert_eq!(issues.len(), 1, "{issues:?}");
    assert_eq!(issues[0].path, "/breakpoints/1/device");
    assert_eq!(issues[0].key, "config.error.unknown_device");
    assert!(issues[0].message.contains("no-such-phone"));
}

#[test]
fn an_explicit_size_still_serializes_and_an_absent_one_stays_absent() {
    let phone = breakpoint(json!({ "id": "phone", "device": "iphone-15" }));
    assert_eq!(
        serde_json::to_value(&phone).expect("serializes"),
        json!({ "id": "phone", "device": "iphone-15", "engine": "chromium", "syncNavigation": true })
    );
    let sized = breakpoint(json!({ "id": "m", "width": 393, "height": 852 }));
    let value = serde_json::to_value(&sized).expect("serializes");
    assert_eq!(
        (value["width"].clone(), value["height"].clone()),
        (json!(393), json!(852))
    );
}
