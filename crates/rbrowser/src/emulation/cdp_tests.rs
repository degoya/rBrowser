use rb_config::{ColorGamut, MediaFeatures, SafeAreaInsets};
use serde_json::json;

use super::*;

fn methods(calls: &[CdpCall]) -> Vec<&'static str> {
    calls.iter().map(|call| call.method).collect()
}

fn call<'a>(calls: &'a [CdpCall], method: &str) -> &'a Value {
    &calls
        .iter()
        .find(|call| call.method == method)
        .unwrap_or_else(|| panic!("{method} was not sent"))
        .params
}

#[test]
fn a_plain_view_on_a_fresh_session_sends_only_what_the_device_toolbar_also_sets() {
    let calls = emulation_calls(None, &Emulation::default(), true);
    assert_eq!(
        methods(&calls),
        [
            "Emulation.setEmulatedMedia",
            "Emulation.setTouchEmulationEnabled",
            "Emulation.setUserAgentOverride",
        ]
    );
    assert_eq!(
        call(&calls, "Emulation.setEmulatedMedia"),
        &json!({ "media": "", "features": [] })
    );
    assert_eq!(
        call(&calls, "Emulation.setUserAgentOverride"),
        &json!({ "userAgent": "" }),
        "empty keeps the engine's user agent"
    );
}

#[test]
fn every_media_feature_maps_to_its_css_name() {
    let next = Emulation {
        color_scheme: Some(ColorScheme::Dark),
        reduced_motion: Some(true),
        media: MediaFeatures {
            reduced_data: Some(true),
            reduced_transparency: Some(false),
            forced_colors: Some(ForcedColors::Active),
            contrast: Some(Contrast::More),
            color_gamut: Some(ColorGamut::P3),
            media_type: Some(MediaType::Print),
        },
        ..Emulation::default()
    };
    let calls = emulation_calls(None, &next, false);
    assert_eq!(
        call(&calls, "Emulation.setEmulatedMedia"),
        &json!({
            "media": "print",
            "features": [
                { "name": "prefers-color-scheme", "value": "dark" },
                { "name": "prefers-reduced-motion", "value": "reduce" },
                { "name": "prefers-reduced-data", "value": "reduce" },
                { "name": "prefers-reduced-transparency", "value": "no-preference" },
                { "name": "forced-colors", "value": "active" },
                { "name": "prefers-contrast", "value": "more" },
                { "name": "color-gamut", "value": "p3" },
            ]
        })
    );
}

#[test]
fn touch_reports_touch_points() {
    let next = Emulation {
        touch: true,
        ..Emulation::default()
    };
    let calls = emulation_calls(None, &next, false);
    assert_eq!(
        call(&calls, "Emulation.setTouchEmulationEnabled"),
        &json!({ "enabled": true, "maxTouchPoints": 5 })
    );
}

#[test]
fn user_agent_platform_and_locale_go_together_with_client_hints() {
    let next = Emulation {
        mobile: true,
        user_agent: Some("Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X)".into()),
        platform: Some("iPhone".into()),
        locale: Some("de-DE".into()),
        ..Emulation::default()
    };
    let calls = emulation_calls(None, &next, false);
    let params = call(&calls, "Emulation.setUserAgentOverride");
    assert_eq!(params["acceptLanguage"], "de-DE,de;q=0.9");
    assert_eq!(params["platform"], "iPhone");
    assert_eq!(params["userAgentMetadata"]["platform"], "iOS");
    assert_eq!(params["userAgentMetadata"]["mobile"], true);
    assert_eq!(
        call(&calls, "Emulation.setLocaleOverride"),
        &json!({ "locale": "de-DE" })
    );
}

#[test]
fn a_locale_without_region_is_its_own_accept_language() {
    assert_eq!(accept_language("fr"), "fr");
    assert_eq!(accept_language("pt-BR"), "pt-BR,pt;q=0.9");
}

#[test]
fn unchanged_groups_send_nothing_and_a_cleared_field_is_cleared() {
    let before = Emulation {
        locale: Some("de-DE".into()),
        text_scale: Some(1.5),
        safe_areas: Some(SafeAreaInsets {
            top: 47,
            ..SafeAreaInsets::default()
        }),
        ..Emulation::default()
    };
    assert!(emulation_calls(Some(&before), &before, false).is_empty());
    let calls = emulation_calls(Some(&before), &Emulation::default(), false);
    assert_eq!(
        call(&calls, "Emulation.setLocaleOverride"),
        &json!({}),
        "no locale clears the override"
    );
    assert_eq!(
        call(&calls, "Page.setFontSizes"),
        &json!({ "fontSizes": { "standard": 16.0, "fixed": 13.0 } })
    );
    assert_eq!(
        call(&calls, "Emulation.setSafeAreaInsetsOverride"),
        &json!({ "insets": {} })
    );
    assert!(
        methods(&calls).contains(&"Emulation.setUserAgentOverride"),
        "the accept language goes with the locale"
    );
}

#[test]
fn text_scale_scales_the_default_font_sizes() {
    let next = Emulation {
        text_scale: Some(1.25),
        ..Emulation::default()
    };
    let calls = emulation_calls(None, &next, false);
    assert_eq!(
        call(&calls, "Page.setFontSizes"),
        &json!({ "fontSizes": { "standard": 20.0, "fixed": 16.0 } })
    );
}

#[test]
fn throttling_enables_the_network_domain_first_and_converts_to_bytes() {
    let next = Emulation {
        network: Some(NetworkConditions {
            offline: false,
            latency_ms: 2000,
            download_kbps: 400,
            upload_kbps: 0,
        }),
        ..Emulation::default()
    };
    let calls = emulation_calls(None, &next, false);
    assert_eq!(
        methods(&calls),
        ["Network.enable", "Network.emulateNetworkConditions"]
    );
    assert_eq!(
        call(&calls, "Network.emulateNetworkConditions"),
        &json!({
            "offline": false,
            "latency": 2000,
            "downloadThroughput": 50000.0,
            "uploadThroughput": -1.0,
        })
    );
    let lifted = emulation_calls(Some(&next), &Emulation::default(), false);
    assert_eq!(
        methods(&lifted),
        ["Network.emulateNetworkConditions"],
        "lifting needs no enable"
    );
    assert_eq!(lifted[0].params["downloadThroughput"], -1.0);
}

#[test]
fn headers_are_sent_as_given_and_cleared_when_gone() {
    let next = Emulation {
        headers: [("X-Feature".to_owned(), "beta".to_owned())].into(),
        ..Emulation::default()
    };
    let calls = emulation_calls(None, &next, true);
    assert!(methods(&calls).contains(&"Network.enable"));
    assert_eq!(
        call(&calls, "Network.setExtraHTTPHeaders"),
        &json!({ "headers": { "X-Feature": "beta" } })
    );
    let cleared = emulation_calls(Some(&next), &Emulation::default(), true);
    assert_eq!(
        call(&cleared, "Network.setExtraHTTPHeaders"),
        &json!({ "headers": {} })
    );
}

#[test]
fn the_orientation_rides_on_the_device_metrics() {
    let metrics = DeviceMetrics {
        width: 852,
        height: 393,
        dpr: 3.0,
        mobile: true,
        scale: 0.5,
    };
    let params = metrics_params(&metrics, Some(Orientation::Landscape));
    assert_eq!(params["width"], 852);
    assert_eq!(
        params["screenOrientation"],
        json!({ "type": "landscapePrimary", "angle": 90 })
    );
    assert!(
        metrics_params(&metrics, None)
            .get("screenOrientation")
            .is_none()
    );
}
