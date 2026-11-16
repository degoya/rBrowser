use rb_config::Viewport;
use serde_json::json;

use super::*;

fn config() -> ProjectConfig {
    serde_json::from_value(json!({
        "version": 1,
        "breakpoints": [
            { "id": "desktop", "width": 1440, "height": 900 },
            { "id": "mobile", "label": "Phone", "width": 393, "height": 852 }
        ]
    }))
    .expect("the test config parses")
}

fn device(id: &str, kind: DeviceKind, width: u32) -> DevicePreset {
    DevicePreset {
        id: id.to_owned(),
        label: Some(format!("Device {id}")),
        kind,
        viewport: Viewport { width, height: 800 },
        dpr: 2.0,
        ua: None,
        platform: None,
        touch: false,
        orientation: None,
        safe_areas: None,
    }
}

fn detected() -> Vec<DetectedBreakpoint> {
    vec![
        DetectedBreakpoint {
            width: 1024,
            kind: BoundaryKind::Min,
            label: Some("lg".to_owned()),
            count: 3,
            source: None,
            heuristic: false,
        },
        DetectedBreakpoint {
            width: 767,
            kind: BoundaryKind::Max,
            label: None,
            count: 1,
            source: None,
            heuristic: false,
        },
    ]
}

#[test]
fn breakpoint_shortcuts_quick_pick_lists_breakpoints_devices_detected_and_free_input() {
    let devices = vec![
        device("laptop", DeviceKind::Desktop, 1280),
        device("phone", DeviceKind::Mobile, 390),
        device("pad", DeviceKind::Tablet, 820),
    ];
    let items = items(
        &config(),
        &devices,
        &detected(),
        Current {
            breakpoint: Some("mobile"),
            width: 393,
        },
    );
    let sections: Vec<Section> = items.iter().map(|item| item.section).collect();
    assert_eq!(
        sections,
        [
            Section::Breakpoints,
            Section::Breakpoints,
            Section::Devices,
            Section::Devices,
            Section::Devices,
            Section::Detected,
            Section::Detected,
            Section::Custom,
        ]
    );
    // Breakpoints by width, the current one checked.
    assert_eq!(items[0].args, json!({ "breakpoint": "mobile" }));
    assert!(items[0].checked);
    assert!(!items[1].checked);
    // Devices: phones, tablets, desktops.
    let devices: Vec<&Value> = items[2..5]
        .iter()
        .map(|item| &item.args["breakpoint"])
        .collect();
    assert_eq!(devices, [&json!("phone"), &json!("pad"), &json!("laptop")]);
    // Detected by width, as a width.
    assert_eq!(items[5].args, json!({ "width": 767 }));
    assert_eq!(items[6].args, json!({ "width": 1024 }));
    assert_eq!(items[7].args, Value::Null);
    assert!(items.iter().all(|item| !item.label.is_empty()));
}

#[test]
fn breakpoint_shortcuts_quick_pick_checks_a_free_size_at_a_detected_width() {
    let items = items(
        &config(),
        &[],
        &detected(),
        Current {
            breakpoint: None,
            width: 767,
        },
    );
    let checked: Vec<&Value> = items
        .iter()
        .filter(|item| item.checked)
        .map(|item| &item.args)
        .collect();
    assert_eq!(checked, [&json!({ "width": 767 })]);
}

#[test]
fn breakpoint_shortcuts_quick_pick_adds_the_view_and_the_new_view_flag() {
    let items = items(&config(), &[], &[], Current::default());
    assert_eq!(
        command_args(&items[0], ViewId(3), false),
        Some(json!({ "breakpoint": "mobile", "view": 3 }))
    );
    assert_eq!(
        command_args(&items[0], ViewId(3), true),
        Some(json!({ "breakpoint": "mobile", "view": 3, "newView": true }))
    );
    let custom = items.last().expect("the free input");
    assert_eq!(command_args(custom, ViewId(3), false), None);
}

#[test]
fn breakpoint_shortcuts_quick_pick_shows_a_preset_only_breakpoint_at_the_presets_size() {
    let mut config = config();
    config.breakpoints.push(
        serde_json::from_value(json!({ "id": "tab", "device": "pad" })).expect("a breakpoint"),
    );
    let devices = vec![device("pad", DeviceKind::Tablet, 820)];
    let items = items(&config, &devices, &[], Current::default());
    assert_eq!(items[1].args, json!({ "breakpoint": "tab" }));
    assert!(
        items[1].label.contains("820") && items[1].label.contains("800"),
        "{}",
        items[1].label
    );
}
