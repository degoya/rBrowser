use rb_config::{DeviceKind, Viewport};
use rb_ipc::BoundaryKind;
use serde_json::json;

use super::*;

fn config() -> ProjectConfig {
    serde_json::from_value(json!({
        "version": 1,
        "breakpoints": [
            { "id": "desktop", "width": 1440, "height": 900 },
            { "id": "mobile", "width": 393, "height": 852 },
            { "id": "tablet", "width": 768, "height": 1024 },
            { "id": "firefox", "width": 1000, "height": 800, "engine": "firefox" },
            { "id": "tablet-wide", "width": 768, "height": 1200 }
        ]
    }))
    .expect("the test config parses")
}

fn ids(breakpoints: &[&Breakpoint]) -> Vec<String> {
    breakpoints.iter().map(|b| b.id.clone()).collect()
}

#[test]
fn breakpoint_shortcuts_sort_chromium_breakpoints_by_width_then_height() {
    let config = config();
    assert_eq!(
        ids(&by_width(&config.breakpoints, &[])),
        ["mobile", "tablet", "tablet-wide", "desktop"]
    );
}

#[test]
fn breakpoint_shortcuts_step_forward_through_every_breakpoint_and_wrap() {
    let config = config();
    let mut current = "mobile".to_owned();
    let mut seen = Vec::new();
    for _ in 0..4 {
        let next = step(&config.breakpoints, &[], Some(&current), 0, true).expect("a breakpoint");
        current = next.id.clone();
        seen.push(current.clone());
    }
    assert_eq!(seen, ["tablet", "tablet-wide", "desktop", "mobile"]);
}

#[test]
fn breakpoint_shortcuts_step_backward_wraps_to_the_widest() {
    let config = config();
    let previous =
        step(&config.breakpoints, &[], Some("mobile"), 393, false).expect("a breakpoint");
    assert_eq!(previous.id, "desktop");
    let previous =
        step(&config.breakpoints, &[], Some("desktop"), 1440, false).expect("a breakpoint");
    assert_eq!(previous.id, "tablet-wide");
}

#[test]
fn breakpoint_shortcuts_step_from_a_free_size_goes_by_its_width() {
    let config = config();
    let next = step(&config.breakpoints, &[], None, 500, true).expect("a breakpoint");
    assert_eq!(next.id, "tablet");
    let previous = step(&config.breakpoints, &[], None, 500, false).expect("a breakpoint");
    assert_eq!(previous.id, "mobile");
    // Past the widest: around to the narrowest.
    let next = step(&config.breakpoints, &[], None, 2000, true).expect("a breakpoint");
    assert_eq!(next.id, "mobile");
    // An id that is no breakpoint (a device preset) counts as a free size.
    let next = step(&config.breakpoints, &[], Some("iphone-15"), 393, true).expect("a breakpoint");
    assert_eq!(next.id, "tablet");
}

#[test]
fn breakpoint_shortcuts_step_without_breakpoints_is_none() {
    assert!(step(&[], &[], None, 500, true).is_none());
}

#[test]
fn breakpoint_shortcuts_nudge_by_exactly_one_px_inside_the_limits() {
    assert_eq!(nudged(767, 1), Some(768));
    assert_eq!(nudged(768, -1), Some(767));
    assert_eq!(nudged(1, -1), None);
    assert_eq!(nudged(MAX_SIDE, 1), None);
}

#[test]
fn breakpoint_shortcuts_parse_the_free_size() {
    let size = |w, h, dpr| {
        Some(FreeSize {
            width: w,
            height: h,
            dpr,
        })
    };
    assert_eq!(parse_free_size("500x800"), size(500, 800, None));
    assert_eq!(parse_free_size(" 500 X 800 "), size(500, 800, None));
    assert_eq!(parse_free_size("500\u{d7}800@2"), size(500, 800, Some(2.0)));
    assert_eq!(parse_free_size("500*800 @ 2.5x"), size(500, 800, Some(2.5)));
    for bad in [
        "",
        "500",
        "x800",
        "500x",
        "0x800",
        "500x0",
        "500x800@0",
        "500x800@11",
        "500x800@abc",
        "99999x800",
        "10001x800",
        "500x800@0.1",
        "-5x800",
        "500x800x2",
    ] {
        assert_eq!(parse_free_size(bad), None, "{bad}");
    }
}

#[test]
fn breakpoint_shortcuts_free_size_ids_round_trip() {
    let size = FreeSize::new(500, 800, Some(2.0)).expect("valid");
    assert_eq!(size.id(), "size-500x800@2");
    assert_eq!(free_size_of_id(&size.id()), Some(size));
    let plain = FreeSize::new(500, 800, None).expect("valid");
    assert_eq!(plain.id(), "size-500x800");
    let breakpoint = size.breakpoint();
    assert_eq!(
        (breakpoint.width, breakpoint.height),
        (Some(500), Some(800))
    );
    assert_eq!(breakpoint.emulation.dpr, Some(2.0));
    assert_eq!(free_size_of_id("mobile"), None);
}

#[test]
fn breakpoint_shortcuts_find_config_breakpoints_devices_and_free_sizes() {
    let config = config();
    let devices = vec![DevicePreset {
        id: "iphone-15".to_owned(),
        label: None,
        kind: DeviceKind::Mobile,
        viewport: Viewport {
            width: 393,
            height: 852,
        },
        dpr: 3.0,
        ua: None,
        platform: None,
        touch: true,
        orientation: None,
        safe_areas: None,
    }];
    let label = |device: &DevicePreset| format!("Device {}", device.id);
    let tablet = find_breakpoint(&config, &devices, "tablet", label).expect("tablet");
    assert_eq!(tablet.width, Some(768));
    let phone = find_breakpoint(&config, &devices, "iphone-15", label).expect("device");
    assert_eq!(phone.device.as_deref(), Some("iphone-15"));
    assert_eq!(phone.label.as_deref(), Some("Device iphone-15"));
    assert_eq!((phone.width, phone.height), (Some(393), Some(852)));
    let free = find_breakpoint(&config, &devices, "size-500x800", label).expect("free");
    assert_eq!((free.width, free.height), (Some(500), Some(800)));
    assert!(find_breakpoint(&config, &devices, "nope", label).is_none());
}

#[test]
fn breakpoint_shortcuts_pick_the_nth_detected_breakpoint_by_width() {
    let found = |width, kind| DetectedBreakpoint {
        width,
        kind,
        label: None,
        count: 1,
        source: None,
        heuristic: false,
    };
    let detected = vec![
        found(1024, BoundaryKind::Min),
        found(640, BoundaryKind::Min),
        found(767, BoundaryKind::Max),
        found(1024, BoundaryKind::Max),
    ];
    assert_eq!(detected_width(&detected, 1), Some(640));
    assert_eq!(detected_width(&detected, 2), Some(767));
    assert_eq!(detected_width(&detected, 3), Some(1024));
    assert_eq!(detected_width(&detected, 4), None);
    assert_eq!(detected_width(&detected, 0), None);
}

#[test]
fn breakpoint_shortcuts_sort_and_step_a_preset_only_breakpoint_by_the_presets_width() {
    let mut config = config();
    config.breakpoints.push(
        serde_json::from_value(json!({ "id": "phone", "device": "pixel" })).expect("a breakpoint"),
    );
    let devices = vec![DevicePreset {
        id: "pixel".to_owned(),
        label: None,
        kind: DeviceKind::Mobile,
        viewport: Viewport {
            width: 412,
            height: 915,
        },
        dpr: 2.625,
        ua: None,
        platform: None,
        touch: true,
        orientation: None,
        safe_areas: None,
    }];
    assert_eq!(
        ids(&by_width(&config.breakpoints, &devices)),
        ["mobile", "phone", "tablet", "tablet-wide", "desktop"]
    );
    assert_eq!(size_of(&config.breakpoints[5], &devices), (412, 915));
    let next = step(&config.breakpoints, &devices, None, 400, true).expect("a breakpoint");
    assert_eq!(next.id, "phone");
}
