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

/// RB-095-02: an Android device is not emulated - its view has the device's own size (the
/// breakpoint `view.add_android` writes), and everything else the emulation asks for is marked.
#[test]
fn android_takes_the_devices_size_and_emulates_nothing_else() {
    let report = EmulationReport::for_engine(EngineKind::Android, &asking_for_much());
    let unsupported = report.unsupported();
    assert!(
        !unsupported.contains(&"viewport".to_owned()),
        "{unsupported:?}"
    );
    assert!(!unsupported.contains(&"dpr".to_owned()), "{unsupported:?}");
    assert_eq!(
        unsupported.len(),
        report.fields.len() - 2,
        "{unsupported:?}"
    );
    let plain = EmulationReport::for_engine(EngineKind::Android, &Emulation::default());
    assert!(plain.unsupported().is_empty());
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

#[test]
fn safari_applies_the_viewport_only_and_says_it_follows() {
    let report = EmulationReport::for_engine(EngineKind::Safari, &Emulation::default());
    let fields: Vec<&str> = report
        .fields
        .iter()
        .map(|field| field.field.as_str())
        .collect();
    assert_eq!(fields, ["viewport", "dpr", LEADER_FIELD]);
    assert_eq!(report.fields[0].support, FieldSupport::Applied);
    assert!(matches!(
        report.fields[1].support,
        FieldSupport::Approximated(_)
    ));
    assert!(matches!(
        &report.fields[2].support,
        FieldSupport::Unsupported(reason) if reason.contains("follows")
    ));
    assert_eq!(report.unsupported(), ["dpr", LEADER_FIELD]);
}

#[test]
fn safari_marks_every_other_field() {
    let report = EmulationReport::for_engine(EngineKind::Safari, &asking_for_much());
    let unsupported = report.unsupported();
    for field in [
        "mobile",
        "touch",
        "platform",
        "reducedMotion",
        "media.forcedColors",
        "media.mediaType",
        "network.offline",
        "network.throttling",
        "headers",
    ] {
        assert!(unsupported.iter().any(|found| found == field), "{field}");
    }
    assert!(!unsupported.iter().any(|found| found == "viewport"));
}

#[test]
fn only_safari_carries_the_leader_marker() {
    for engine in [
        EngineKind::Chromium,
        EngineKind::Firefox,
        EngineKind::Webkit,
    ] {
        let report = EmulationReport::for_engine(engine, &Emulation::default());
        assert!(
            !report
                .fields
                .iter()
                .any(|field| field.field == LEADER_FIELD),
            "{engine:?}"
        );
    }
}

#[test]
fn ios_shows_the_device_s_own_size_and_follows() {
    let report = EmulationReport::for_engine(EngineKind::Ios, &asking_for_much());
    let support = |name: &str| {
        report
            .fields
            .iter()
            .find(|field| field.field == name)
            .map(|field| field.support.clone())
    };
    // Width and pixel ratio are the device's: marked, never emulated (RB-095-04).
    assert!(matches!(
        support("viewport"),
        Some(FieldSupport::Unsupported(reason)) if reason.contains("device")
    ));
    assert!(matches!(support("dpr"), Some(FieldSupport::Unsupported(_))));
    // A phone is mobile and takes touch by itself.
    assert_eq!(support("mobile"), Some(FieldSupport::Applied));
    assert_eq!(support("touch"), Some(FieldSupport::Applied));
    for field in ["platform", "reducedMotion", "media.mediaType", "headers"] {
        assert!(
            matches!(support(field), Some(FieldSupport::Unsupported(_))),
            "{field}"
        );
    }
    assert!(matches!(
        support(LEADER_FIELD),
        Some(FieldSupport::Unsupported(reason)) if reason.contains("follows")
    ));
}
