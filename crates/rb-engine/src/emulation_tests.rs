use super::*;

#[test]
fn emulation_serializes_camel_case() {
    let emulation = Emulation {
        width: 393,
        height: 852,
        dpr: 3.0,
        mobile: true,
        touch: true,
        locale: Some("de-DE".into()),
        color_scheme: Some(ColorScheme::Dark),
        ..Emulation::default()
    };
    let json = serde_json::to_value(&emulation).expect("serializes");
    assert_eq!(json["width"], 393);
    assert_eq!(json["colorScheme"], "dark");
    assert_eq!(json["locale"], "de-DE");
    assert!(
        json.get("userAgent").is_none(),
        "absent fields are left out"
    );
}

#[test]
fn the_default_is_a_plain_desktop_view() {
    let emulation = Emulation::default();
    assert_eq!(emulation.dpr, 1.0);
    assert!(!emulation.mobile && !emulation.touch);
    assert_eq!(emulation.scale, None);
}

#[test]
fn emulation_set_args_read_the_mcp_shape() {
    let args: EmulationSetArgs = serde_json::from_value(serde_json::json!({
        "target": "all",
        "fields": { "colorScheme": "dark", "network": "slow-3g" }
    }))
    .expect("parses");
    assert_eq!(args.target, Some(Target::All));
    assert_eq!(args.fields.color_scheme, Some(ColorScheme::Dark));
    let active: EmulationSetArgs =
        serde_json::from_value(serde_json::json!({ "fields": {} })).expect("parses");
    assert_eq!(active.target, None);
    let change = EmulationChange {
        views: vec![ViewId(2)],
        reload_required: false,
    };
    assert_eq!(
        serde_json::to_value(&change).expect("serializes"),
        serde_json::json!({ "views": [2], "reloadRequired": false })
    );
}

fn breakpoint(json: serde_json::Value) -> Breakpoint {
    serde_json::from_value(json).expect("a breakpoint")
}

fn iphone() -> DevicePreset {
    serde_json::from_value(serde_json::json!({
        "id": "iphone-15",
        "kind": "mobile",
        "viewport": { "width": 393, "height": 852 },
        "dpr": 3.0,
        "ua": "Mozilla/5.0 (iPhone)",
        "platform": "iPhone",
        "touch": true,
        "orientation": "portrait",
        "safeAreas": { "top": 59, "bottom": 34 }
    }))
    .expect("a device")
}

#[test]
fn a_plain_breakpoint_resolves_to_its_size_and_the_display_dpr() {
    let resolved = resolve_emulation(
        &breakpoint(serde_json::json!({ "id": "m", "width": 393, "height": 852 })),
        &EmulationFields::default(),
        &[],
    )
    .expect("resolves");
    assert_eq!((resolved.width, resolved.height), (393, 852));
    assert_eq!(resolved.dpr, 0.0, "0 keeps the display's DPR");
    assert!(!resolved.touch && !resolved.mobile);
    assert_eq!(resolved.user_agent, None);
    assert_eq!(resolved.network, None);
    assert!(resolved.headers.is_empty());
}

#[test]
fn own_fields_win_over_the_device_and_the_device_over_the_defaults() {
    let defaults = EmulationFields {
        dpr: Some(1.5),
        locale: Some("en-GB".into()),
        color_scheme: Some(ColorScheme::Light),
        user_agent: Some("Default UA".into()),
        ..EmulationFields::default()
    };
    let resolved = resolve_emulation(
        &breakpoint(serde_json::json!({
            "id": "m", "width": 393, "height": 852, "device": "iphone-15",
            "colorScheme": "dark", "platform": "MyPlatform"
        })),
        &defaults,
        &[iphone()],
    )
    .expect("resolves");
    assert_eq!(resolved.dpr, 3.0, "the device's DPR over the default");
    assert!(resolved.touch && resolved.mobile, "a mobile device");
    assert_eq!(resolved.user_agent.as_deref(), Some("Mozilla/5.0 (iPhone)"));
    assert_eq!(resolved.platform.as_deref(), Some("MyPlatform"));
    assert_eq!(
        resolved.locale.as_deref(),
        Some("en-GB"),
        "from the defaults"
    );
    assert_eq!(resolved.color_scheme, Some(ColorScheme::Dark));
    assert_eq!(resolved.orientation, Some(Orientation::Portrait));
    assert_eq!(resolved.safe_areas.map(|insets| insets.top), Some(59));
}

#[test]
fn an_unknown_device_is_an_error() {
    let result = resolve_emulation(
        &breakpoint(serde_json::json!({ "id": "m", "width": 1, "height": 1, "device": "nope" })),
        &EmulationFields::default(),
        &[iphone()],
    );
    assert!(matches!(result, Err(EngineError::Invalid(_))));
}

#[test]
fn mobile_follows_touch_unless_set() {
    let touch_only = resolve_emulation(
        &breakpoint(serde_json::json!({ "id": "t", "width": 768, "height": 1024, "touch": true })),
        &EmulationFields::default(),
        &[],
    )
    .expect("resolves");
    assert!(touch_only.mobile);
    let desktop_touch = resolve_emulation(
        &breakpoint(serde_json::json!({
            "id": "t", "width": 1280, "height": 800, "touch": true, "mobile": false
        })),
        &EmulationFields::default(),
        &[],
    )
    .expect("resolves");
    assert!(!desktop_touch.mobile);
}

#[test]
fn media_features_merge_field_by_field() {
    let defaults = EmulationFields {
        media: Some(MediaFeatures {
            reduced_data: Some(true),
            contrast: Some(rb_config::Contrast::More),
            ..MediaFeatures::default()
        }),
        ..EmulationFields::default()
    };
    let resolved = resolve_emulation(
        &breakpoint(serde_json::json!({
            "id": "p", "width": 800, "height": 600,
            "media": { "mediaType": "print", "contrast": "less" }
        })),
        &defaults,
        &[],
    )
    .expect("resolves");
    assert_eq!(resolved.media.reduced_data, Some(true));
    assert_eq!(resolved.media.contrast, Some(rb_config::Contrast::Less));
    assert_eq!(resolved.media.media_type, Some(rb_config::MediaType::Print));
}

#[test]
fn network_presets_expand_to_conditions() {
    let expand = |preset: &str| {
        resolve_emulation(
            &breakpoint(serde_json::json!({
                "id": "n", "width": 800, "height": 600, "network": preset
            })),
            &EmulationFields::default(),
            &[],
        )
        .expect("resolves")
        .network
        .expect("throttled")
    };
    assert!(expand("offline").offline);
    let slow = expand("slow-3g");
    assert_eq!(
        (slow.latency_ms, slow.download_kbps, slow.upload_kbps),
        (2000, 400, 400)
    );
    let fast = expand("fast-3g");
    assert_eq!(
        (fast.latency_ms, fast.download_kbps, fast.upload_kbps),
        (563, 1440, 675)
    );
    let four_g = expand("4g");
    assert_eq!(
        (four_g.latency_ms, four_g.download_kbps, four_g.upload_kbps),
        (165, 8100, 1350)
    );
    let custom = resolve_emulation(
        &breakpoint(serde_json::json!({
            "id": "n", "width": 800, "height": 600,
            "network": { "latencyMs": 10, "downloadKbps": 100 }
        })),
        &EmulationFields::default(),
        &[],
    )
    .expect("resolves")
    .network
    .expect("throttled");
    assert_eq!((custom.latency_ms, custom.download_kbps), (10, 100));
}

#[test]
fn headers_of_the_breakpoint_override_the_defaults_by_name() {
    let defaults = EmulationFields {
        headers: [
            ("X-Team".to_owned(), "web".to_owned()),
            ("X-Env".to_owned(), "dev".to_owned()),
        ]
        .into(),
        ..EmulationFields::default()
    };
    let resolved = resolve_emulation(
        &breakpoint(serde_json::json!({
            "id": "h", "width": 800, "height": 600, "headers": { "X-Env": "stage" }
        })),
        &defaults,
        &[],
    )
    .expect("resolves");
    assert_eq!(resolved.headers["X-Team"], "web");
    assert_eq!(resolved.headers["X-Env"], "stage");
}

#[test]
fn the_orientation_turns_the_viewport() {
    let landscape = resolve_emulation(
        &breakpoint(serde_json::json!({
            "id": "l", "width": 393, "height": 852, "orientation": "landscape"
        })),
        &EmulationFields::default(),
        &[],
    )
    .expect("resolves");
    assert_eq!((landscape.width, landscape.height), (852, 393));
    let portrait = resolve_emulation(
        &breakpoint(serde_json::json!({
            "id": "p", "width": 852, "height": 393, "orientation": "portrait"
        })),
        &EmulationFields::default(),
        &[],
    )
    .expect("resolves");
    assert_eq!((portrait.width, portrait.height), (393, 852));
}

#[test]
fn values_a_browser_would_refuse_are_errors() {
    let invalid = |json: serde_json::Value| {
        matches!(
            resolve_emulation(&breakpoint(json), &EmulationFields::default(), &[]),
            Err(EngineError::Invalid(_))
        )
    };
    let base = |extra: serde_json::Value| {
        let mut value = serde_json::json!({ "id": "x", "width": 800, "height": 600 });
        if let (Some(object), Some(extra)) = (value.as_object_mut(), extra.as_object()) {
            object.extend(extra.clone());
        }
        value
    };
    assert!(invalid(base(serde_json::json!({ "dpr": 0.0 }))));
    assert!(invalid(base(serde_json::json!({ "dpr": 11.0 }))));
    assert!(invalid(base(serde_json::json!({ "textScale": -1.0 }))));
    assert!(invalid(base(
        serde_json::json!({ "locale": "de-DE\r\nX: 1" })
    )));
    assert!(invalid(base(serde_json::json!({ "userAgent": "a\nb" }))));
    assert!(invalid(base(
        serde_json::json!({ "headers": { "Bad Name": "v" } })
    )));
    assert!(invalid(base(
        serde_json::json!({ "headers": { "X-Ok": "a\r\nb" } })
    )));
    assert!(!invalid(base(
        serde_json::json!({ "locale": "de-DE", "textScale": 1.25 })
    )));
}

#[test]
fn an_empty_user_agent_keeps_the_engines() {
    let resolved = resolve_emulation(
        &breakpoint(serde_json::json!({ "id": "u", "width": 800, "height": 600, "userAgent": "" })),
        &EmulationFields::default(),
        &[],
    )
    .expect("resolves");
    assert_eq!(resolved.user_agent, None);
}

#[test]
fn a_breakpoint_with_only_a_device_takes_the_presets_viewport() {
    let resolved = resolve_emulation(
        &breakpoint(serde_json::json!({ "id": "phone", "device": "iphone-15" })),
        &EmulationFields::default(),
        &[iphone()],
    )
    .expect("resolves");
    assert_eq!((resolved.width, resolved.height), (393, 852));
    assert_eq!(resolved.dpr, 3.0);
    assert!(resolved.touch && resolved.mobile);
}

#[test]
fn a_size_on_the_breakpoint_overrides_the_presets_viewport() {
    let resolved = resolve_emulation(
        &breakpoint(serde_json::json!({ "id": "wide", "device": "iphone-15", "width": 430 })),
        &EmulationFields::default(),
        &[iphone()],
    )
    .expect("resolves");
    assert_eq!((resolved.width, resolved.height), (430, 852));
    let turned = resolve_emulation(
        &breakpoint(serde_json::json!({
            "id": "turned", "device": "iphone-15", "orientation": "landscape"
        })),
        &EmulationFields::default(),
        &[iphone()],
    )
    .expect("resolves");
    assert_eq!((turned.width, turned.height), (852, 393));
}

#[test]
fn a_breakpoint_without_size_or_device_does_not_resolve() {
    let result = resolve_emulation(
        &breakpoint(serde_json::json!({ "id": "bare" })),
        &EmulationFields::default(),
        &[iphone()],
    );
    assert!(
        matches!(&result, Err(EngineError::Invalid(message)) if message.contains("bare")),
        "{result:?}"
    );
}
