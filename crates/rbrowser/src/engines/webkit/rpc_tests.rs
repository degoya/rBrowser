use std::collections::BTreeMap;

use rb_config::{
    ColorScheme, Contrast, ForcedColors, Geolocation, MediaFeatures, MediaType, NetworkConditions,
};
use rb_engine::{Emulation, EngineError, ViewId};
use rb_sidecar::SidecarError;
use serde_json::json;

use super::*;

fn remote(code: i64, message: &str) -> SidecarError {
    SidecarError::Remote {
        method: CREATE.to_owned(),
        code,
        message: message.to_owned(),
        data: None,
    }
}

#[test]
fn a_phone_breakpoint_becomes_playwright_context_options() {
    let emulation = Emulation {
        width: 390,
        height: 844,
        dpr: 3.0,
        mobile: true,
        touch: true,
        reduced_motion: Some(true),
        color_scheme: Some(ColorScheme::Dark),
        locale: Some("de-DE".to_owned()),
        user_agent: Some("Mozilla/5.0 (iPhone)".to_owned()),
        ..Emulation::default()
    };
    assert_eq!(
        context_options(&emulation),
        json!({
            "viewport": { "width": 390, "height": 844 },
            "deviceScaleFactor": 3.0,
            "isMobile": true,
            "hasTouch": true,
            "userAgent": "Mozilla/5.0 (iPhone)",
            "locale": "de-DE",
            "colorScheme": "dark",
            "reducedMotion": "reduce",
        })
    );
}

/// In its grid cell (RB-080-40) the context takes the cell's height and factor; the
/// breakpoint's DPR gives way (Playwright's one knob), whatever it was.
#[test]
fn in_a_cell_the_context_renders_the_breakpoint_width_at_the_cells_factor() {
    let emulation = Emulation {
        width: 768,
        height: 1024,
        dpr: 2.0,
        ..Emulation::default()
    };
    let cell = rb_engine::external::CellScale {
        factor: 0.82,
        width: 768,
        height: 610,
    };
    let options = context_options_in(&emulation, Some(cell));
    assert_eq!(options["viewport"], json!({ "width": 768, "height": 610 }));
    assert_eq!(options["deviceScaleFactor"], json!(0.82));
    assert_eq!(
        context_options_in(&emulation, None),
        context_options(&emulation)
    );
    let display = Emulation {
        dpr: 0.0,
        ..emulation
    };
    assert_eq!(
        context_options_in(&display, Some(cell))["deviceScaleFactor"],
        json!(0.82),
        "a breakpoint without a DPR is scaled all the same"
    );
}

/// DPR 0 is the display's ratio: no `deviceScaleFactor`, so Playwright keeps the screen's.
#[test]
fn the_display_pixel_ratio_sends_no_device_scale_factor() {
    let emulation = Emulation {
        width: 1280,
        height: 800,
        dpr: 0.0,
        ..Emulation::default()
    };
    assert!(
        context_options(&emulation)
            .get("deviceScaleFactor")
            .is_none()
    );
}

#[test]
fn time_zone_and_position_become_context_options() {
    let emulation = Emulation {
        timezone: Some("Europe/Berlin".to_owned()),
        geolocation: Some(Geolocation {
            latitude: 52.52,
            longitude: 13.405,
            accuracy: 50.0,
        }),
        cpu_throttling: Some(4.0),
        ..Emulation::default()
    };
    let options = context_options(&emulation);
    assert_eq!(options["timezoneId"], "Europe/Berlin");
    assert_eq!(
        options["geolocation"],
        json!({ "latitude": 52.52, "longitude": 13.405, "accuracy": 50.0 })
    );
    assert_eq!(options["permissions"], json!(["geolocation"]));
    // No Playwright option throttles WebKit's CPU: the report marks it, nothing is sent.
    assert!(options.get("cpuThrottling").is_none());
}

#[test]
fn media_network_and_headers_map_where_playwright_has_an_option() {
    let emulation = Emulation {
        media: MediaFeatures {
            forced_colors: Some(ForcedColors::Active),
            contrast: Some(Contrast::More),
            media_type: Some(MediaType::Print),
            ..MediaFeatures::default()
        },
        network: Some(NetworkConditions {
            offline: true,
            latency_ms: 400,
            download_kbps: 0,
            upload_kbps: 0,
        }),
        headers: BTreeMap::from([("X-Team".to_owned(), "web".to_owned())]),
        ..Emulation::default()
    };
    let options = context_options(&emulation);
    assert_eq!(options["forcedColors"], "active");
    assert_eq!(options["contrast"], "more");
    assert_eq!(options["media"], "print");
    assert_eq!(options["offline"], true);
    assert_eq!(options["extraHTTPHeaders"], json!({ "X-Team": "web" }));
    // Throttling has no Playwright option for WebKit: the report marks it, nothing is sent.
    assert!(options.get("latency").is_none());
}

#[test]
fn a_contrast_playwright_does_not_know_is_left_out() {
    let emulation = Emulation {
        media: MediaFeatures {
            contrast: Some(Contrast::Less),
            ..MediaFeatures::default()
        },
        ..Emulation::default()
    };
    assert!(context_options(&emulation).get("contrast").is_none());
}

#[test]
fn sidecar_errors_map_to_engine_errors() {
    assert!(matches!(
        engine_error(
            CREATE,
            remote(NOT_INSTALLED, "WebKit 2388 is not installed")
        ),
        EngineError::Unavailable {
            engine: rb_config::EngineKind::Webkit,
            ..
        }
    ));
    assert!(matches!(
        engine_error(NAVIGATE, remote(-32005, "no WebKit view 3")),
        EngineError::Closed
    ));
    assert!(matches!(
        engine_error(
            NAVIGATE,
            remote(-32602, "url scheme javascript: is not allowed")
        ),
        EngineError::Invalid(_)
    ));
    assert!(matches!(
        engine_error(NAVIGATE, SidecarError::Exited),
        EngineError::Closed
    ));
    let other = engine_error(
        RELOAD,
        SidecarError::Timeout {
            method: RELOAD.to_owned(),
            ms: 30_000,
        },
    );
    assert!(other.to_string().contains("webkit.view.reload"), "{other}");
}

#[test]
fn notifications_parse() {
    let event: EventNote =
        serde_json::from_value(json!({ "view": 3, "kind": "navigated", "url": "http://a/" }))
            .expect("parses");
    assert_eq!(event.view, Some(ViewId(3)));
    assert_eq!(event.url.as_deref(), Some("http://a/"));
    let gone: EventNote =
        serde_json::from_value(json!({ "kind": "disconnected" })).expect("parses");
    assert_eq!(gone.view, None);
    let report: AgentNote =
        serde_json::from_value(json!({ "view": 1, "top": true, "payload": "{}" })).expect("parses");
    assert!(report.top);
}
