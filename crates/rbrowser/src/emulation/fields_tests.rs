use rb_config::{ColorScheme, Contrast, MediaType, NetworkPreset, NetworkThrottle};
use serde_json::{Value, json};

use super::*;

fn breakpoint(value: Value) -> Breakpoint {
    serde_json::from_value(value).expect("a breakpoint")
}

fn headers(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
        .collect()
}

#[test]
fn an_overlay_keeps_what_the_top_leaves_open() {
    let base: EmulationFields = serde_json::from_value(json!({
        "touch": true, "colorScheme": "light", "media": { "contrast": "more" },
        "headers": { "A": "1", "B": "2" }
    }))
    .expect("fields");
    let top: EmulationFields = serde_json::from_value(json!({
        "colorScheme": "dark", "network": "slow-3g", "media": { "mediaType": "print" },
        "headers": { "B": "3" }
    }))
    .expect("fields");
    let merged = overlay(&base, &top);
    assert_eq!(merged.touch, Some(true));
    assert_eq!(merged.color_scheme, Some(ColorScheme::Dark));
    assert_eq!(
        merged.network,
        Some(NetworkThrottle::Preset(NetworkPreset::Slow3g))
    );
    let media = merged.media.expect("media");
    assert_eq!(media.contrast, Some(Contrast::More));
    assert_eq!(media.media_type, Some(MediaType::Print));
    assert_eq!(merged.headers, headers(&[("A", "1"), ("B", "3")]));
}

#[test]
fn a_view_resolves_its_breakpoint_with_the_live_overrides() {
    let mobile = breakpoint(json!({ "id": "m", "width": 393, "height": 852, "touch": true }));
    let overrides = EmulationFields {
        color_scheme: Some(ColorScheme::Dark),
        ..EmulationFields::default()
    };
    let emulation = view_emulation(
        Some(&mobile),
        &Emulation::default(),
        &EmulationFields::default(),
        &overrides,
        &[],
    )
    .expect("resolves");
    assert_eq!((emulation.width, emulation.height), (393, 852));
    assert!(emulation.touch);
    assert_eq!(emulation.color_scheme, Some(ColorScheme::Dark));
}

#[test]
fn a_free_sized_view_keeps_its_size_and_dpr() {
    let current = Emulation {
        width: 500,
        height: 700,
        dpr: 2.0,
        ..Emulation::default()
    };
    let overrides = EmulationFields {
        reduced_motion: Some(true),
        ..EmulationFields::default()
    };
    let emulation = view_emulation(None, &current, &EmulationFields::default(), &overrides, &[])
        .expect("resolves");
    assert_eq!((emulation.width, emulation.height), (500, 700));
    assert_eq!(emulation.dpr, 2.0);
    assert_eq!(emulation.reduced_motion, Some(true));
}

#[test]
fn only_server_visible_fields_need_a_reload() {
    let before = Emulation::default();
    let dark = Emulation {
        color_scheme: Some(ColorScheme::Dark),
        touch: true,
        ..Emulation::default()
    };
    assert!(!reload_required(&before, &dark));
    let german = Emulation {
        locale: Some("de-DE".into()),
        ..Emulation::default()
    };
    assert!(reload_required(&before, &german));
    let with_header = Emulation {
        headers: headers(&[("X", "1")]),
        ..Emulation::default()
    };
    assert!(reload_required(&before, &with_header));
}

#[test]
fn the_shell_never_sees_a_header_value() {
    let emulation = Emulation {
        headers: headers(&[("Authorization", "Bearer secret")]),
        ..Emulation::default()
    };
    let shown = masked(&emulation);
    assert_eq!(shown.headers["Authorization"], MASKED_VALUE);
    assert!(
        !serde_json::to_string(&shown)
            .expect("json")
            .contains("secret")
    );
}

#[test]
fn an_untrusted_workspace_sends_only_live_headers() {
    let resolved = headers(&[
        ("X-Project", "from config.json"),
        ("Authorization", "from config.local.json"),
        ("X-Live", "set"),
        ("X-Shadowed", "project"),
    ]);
    let live = headers(&[("X-Live", "set"), ("X-Shadowed", "live")]);
    assert_eq!(
        allowed_headers(&resolved, &live),
        headers(&[("X-Live", "set")])
    );
}

#[test]
fn the_extras_merge_breakpoint_device_defaults_and_live_overrides() {
    let phone: rb_config::DevicePreset = serde_json::from_value(json!({
        "id": "phone", "kind": "mobile", "viewport": { "width": 390, "height": 844 },
        "dpr": 3.0, "touch": true
    }))
    .expect("a device");
    let breakpoint = breakpoint(json!({
        "id": "m", "device": "phone",
        "geolocation": { "latitude": 52.52, "longitude": 13.4 },
        "cpuThrottling": 4
    }));
    let defaults: EmulationFields = serde_json::from_value(json!({
        "timezone": "Europe/Berlin", "offline": true
    }))
    .expect("fields");
    let resolve = |overrides: &EmulationFields| {
        view_emulation(
            Some(&breakpoint),
            &Emulation::default(),
            &defaults,
            overrides,
            std::slice::from_ref(&phone),
        )
        .expect("resolves")
    };
    let plain = resolve(&EmulationFields::default());
    assert_eq!((plain.width, plain.height, plain.dpr), (390, 844, 3.0));
    assert!(plain.touch, "the device's");
    assert_eq!(plain.geolocation.map(|geo| geo.latitude), Some(52.52));
    assert_eq!(plain.timezone.as_deref(), Some("Europe/Berlin"));
    assert_eq!(plain.cpu_throttling, Some(4.0));
    assert!(plain.network.is_some_and(|network| network.offline));
    let live: EmulationFields = serde_json::from_value(json!({
        "geolocation": null, "timezone": "Asia/Tokyo", "cpuThrottling": 1, "offline": false
    }))
    .expect("fields");
    let switched = resolve(&live);
    assert_eq!(
        switched.geolocation, None,
        "null switches the breakpoint's off"
    );
    assert_eq!(switched.timezone.as_deref(), Some("Asia/Tokyo"));
    assert_eq!(switched.cpu_throttling, None);
    assert_eq!(switched.network, None);
    let print: EmulationFields = serde_json::from_value(json!({
        "paper": "A4", "media": { "mediaType": "print" }
    }))
    .expect("fields");
    let merged = overlay(&live, &print);
    assert_eq!(
        merged.geolocation,
        Some(None),
        "an overlay keeps an explicit off"
    );
    assert_eq!(merged.paper, Some(rb_config::Paper::A4));
}

#[test]
fn dir_and_lang_overlay_live_and_need_no_reload() {
    let base: EmulationFields =
        serde_json::from_value(json!({ "dir": "ltr", "lang": "en" })).expect("fields");
    let top: EmulationFields = serde_json::from_value(json!({ "dir": "rtl" })).expect("fields");
    let merged = overlay(&base, &top);
    assert_eq!(merged.dir, Some(rb_config::TextDirection::Rtl));
    assert_eq!(merged.lang.as_deref(), Some("en"), "the base's lang stays");
    let before = Emulation::default();
    let after = Emulation {
        dir: Some(rb_config::TextDirection::Rtl),
        lang: Some("ar".into()),
        ..Emulation::default()
    };
    assert!(
        !reload_required(&before, &after),
        "the agent sets dir and lang on the live document"
    );
}
