use std::collections::BTreeMap;

use rb_config::{ForcedColors, MediaFeatures, NetworkConditions};

use super::*;

fn asking_for_much() -> Emulation {
    Emulation {
        mobile: true,
        touch: true,
        platform: Some("iPhone".into()),
        reduced_motion: Some(true),
        media: MediaFeatures {
            forced_colors: Some(ForcedColors::Active),
            media_type: Some(MediaType::Print),
            ..MediaFeatures::default()
        },
        network: Some(NetworkConditions {
            offline: true,
            latency_ms: 400,
            download_kbps: 0,
            upload_kbps: 0,
        }),
        headers: BTreeMap::from([("X-Test".to_owned(), "1".to_owned())]),
        ..Emulation::default()
    }
}

#[test]
fn chromium_applies_everything() {
    let report = EmulationReport::for_engine(EngineKind::Chromium, &asking_for_much());
    assert!(report.unsupported().is_empty());
    assert!(
        report
            .fields
            .iter()
            .all(|field| field.support == FieldSupport::Applied)
    );
}

#[test]
fn a_default_emulation_reports_viewport_and_dpr_only() {
    let report = EmulationReport::for_engine(EngineKind::Firefox, &Emulation::default());
    let fields: Vec<&str> = report
        .fields
        .iter()
        .map(|field| field.field.as_str())
        .collect();
    assert_eq!(fields, ["viewport", "dpr"]);
    assert!(report.unsupported().is_empty());
}

#[test]
fn firefox_lists_what_bidi_cannot_emulate() {
    let report = EmulationReport::for_engine(EngineKind::Firefox, &asking_for_much());
    assert_eq!(
        report.unsupported(),
        [
            "mobile",
            "touch",
            "platform",
            "reducedMotion",
            "media.forcedColors",
            "media.mediaType",
            "network.throttling",
        ]
    );
    let offline = report
        .fields
        .iter()
        .find(|field| field.field == "network.offline")
        .expect("offline is asked for");
    assert_eq!(offline.support, FieldSupport::Applied);
}

#[test]
fn webkit_lists_what_playwright_cannot_emulate() {
    let report = EmulationReport::for_engine(EngineKind::Webkit, &asking_for_much());
    assert_eq!(report.unsupported(), ["platform", "network.throttling"]);
}

#[test]
fn a_report_serializes_per_field() {
    let report = EmulationReport::for_engine(EngineKind::Firefox, &asking_for_much());
    let json = serde_json::to_value(&report).expect("serializes");
    assert_eq!(
        json["fields"][0],
        serde_json::json!({ "field": "viewport", "support": "applied" })
    );
    let touch = json["fields"]
        .as_array()
        .and_then(|fields| fields.iter().find(|field| field["field"] == "touch"))
        .expect("touch");
    assert_eq!(touch["support"], "unsupported");
    assert!(
        touch["reason"]
            .as_str()
            .is_some_and(|reason| reason.contains("BiDi"))
    );
}

/// RB-080-10's extras: Chromium applies them over CDP, WebKit the time zone and position as
/// Playwright context options; Firefox has no BiDi override in rBrowser for any of them.
#[test]
fn time_zone_position_and_cpu_are_reported_per_engine() {
    let emulation = Emulation {
        timezone: Some("Europe/Berlin".into()),
        geolocation: Some(rb_config::Geolocation {
            latitude: 52.52,
            longitude: 13.405,
            accuracy: 50.0,
        }),
        cpu_throttling: Some(4.0),
        ..Emulation::default()
    };
    let fields: Vec<String> = EmulationReport::for_engine(EngineKind::Chromium, &emulation)
        .fields
        .into_iter()
        .map(|field| field.field)
        .collect();
    assert_eq!(
        fields,
        [
            "viewport",
            "dpr",
            "geolocation",
            "timezone",
            "cpuThrottling"
        ]
    );
    assert!(
        EmulationReport::for_engine(EngineKind::Chromium, &emulation)
            .unsupported()
            .is_empty()
    );
    assert_eq!(
        EmulationReport::for_engine(EngineKind::Webkit, &emulation).unsupported(),
        ["cpuThrottling"]
    );
    assert_eq!(
        EmulationReport::for_engine(EngineKind::Firefox, &emulation).unsupported(),
        ["geolocation", "timezone", "cpuThrottling"]
    );
}
