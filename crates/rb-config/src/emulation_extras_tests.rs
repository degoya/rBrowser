use serde_json::json;

use super::*;
use crate::{Breakpoint, EmulationFields};

fn fields(value: serde_json::Value) -> EmulationFields {
    serde_json::from_value(value).expect("valid emulation fields")
}

#[test]
fn the_new_fields_read_and_write_camel_case() {
    let fields = fields(json!({
        "geolocation": { "latitude": 52.52, "longitude": 13.4, "accuracy": 50 },
        "timezone": "America/New_York",
        "cpuThrottling": 4,
        "offline": true,
        "paper": "A4",
    }));
    assert_eq!(
        fields.geolocation,
        Some(Some(Geolocation {
            latitude: 52.52,
            longitude: 13.4,
            accuracy: 50.0,
        }))
    );
    assert_eq!(fields.timezone.as_deref(), Some("America/New_York"));
    assert_eq!(fields.cpu_throttling, Some(4.0));
    assert_eq!(fields.offline, Some(true));
    assert_eq!(fields.paper, Some(Paper::A4));
    let back = serde_json::to_value(&fields).expect("serializes");
    assert_eq!(back["cpuThrottling"], 4.0);
    assert_eq!(back["paper"], "A4");
    assert_eq!(back["geolocation"]["latitude"], 52.52);
}

#[test]
fn geolocation_null_is_an_explicit_off_and_absent_is_unset() {
    assert_eq!(
        fields(json!({ "geolocation": null })).geolocation,
        Some(None)
    );
    assert_eq!(fields(json!({})).geolocation, None);
    let off = EmulationFields {
        geolocation: Some(None),
        ..EmulationFields::default()
    };
    assert_eq!(
        serde_json::to_value(&off).expect("serializes"),
        json!({ "geolocation": null })
    );
    assert_eq!(
        serde_json::to_value(EmulationFields::default()).expect("serializes"),
        json!({})
    );
}

#[test]
fn accuracy_defaults_to_50_m() {
    let fields = fields(json!({ "geolocation": { "latitude": 1, "longitude": 2 } }));
    assert_eq!(
        fields.geolocation.flatten().map(|geo| geo.accuracy),
        Some(DEFAULT_ACCURACY_M)
    );
}

#[test]
fn geolocation_ranges() {
    let at = |latitude, longitude, accuracy| Geolocation {
        latitude,
        longitude,
        accuracy,
    };
    for (geo, ok) in [
        (at(52.52, 13.4, 50.0), true),
        (at(-90.0, -180.0, 0.0), true),
        (at(90.0, 180.0, 0.0), true),
        (at(90.1, 0.0, 0.0), false),
        (at(0.0, -180.5, 0.0), false),
        (at(0.0, 0.0, -1.0), false),
        (at(f64::NAN, 0.0, 0.0), false),
        (at(0.0, f64::INFINITY, 0.0), false),
    ] {
        assert_eq!(geo.problem().is_none(), ok, "{geo:?}");
    }
}

#[test]
fn time_zones_are_iana_names() {
    for (name, ok) in [
        ("America/New_York", true),
        ("Europe/Berlin", true),
        ("Asia/Tokyo", true),
        ("Australia/Sydney", true),
        ("UTC", true),
        ("Etc/GMT+5", true),
        ("US/Eastern", true),
        ("america/new_york", false),
        ("Mars/Olympus_Mons", false),
        ("", false),
        ("# IANA", false),
        ("Europe/Berlin\n", false),
    ] {
        assert_eq!(is_iana_timezone(name), ok, "{name:?}");
    }
}

#[test]
fn cpu_throttling_is_a_factor_from_1_to_100() {
    for (rate, ok) in [
        (1.0, true),
        (4.0, true),
        (100.0, true),
        (0.5, false),
        (100.5, false),
        (f64::NAN, false),
    ] {
        assert_eq!(cpu_throttling_problem(rate).is_none(), ok, "{rate}");
    }
}

#[test]
fn paper_sizes_at_96_dpi() {
    assert_eq!(
        Paper::A4.viewport(),
        Viewport {
            width: 794,
            height: 1123
        }
    );
    assert_eq!(
        Paper::Letter.viewport(),
        Viewport {
            width: 816,
            height: 1056
        }
    );
}

#[test]
fn check_reports_every_bad_extra_with_its_pointer() {
    let fields = fields(json!({
        "geolocation": { "latitude": 91, "longitude": 0 },
        "timezone": "Mars/Olympus_Mons",
        "cpuThrottling": 0.5,
    }));
    let issues = check_extras(&fields, "/breakpoints/2");
    let found: Vec<(&str, &str)> = issues
        .iter()
        .map(|issue| (issue.path.as_str(), issue.key.as_str()))
        .collect();
    assert_eq!(
        found,
        vec![
            (
                "/breakpoints/2/geolocation",
                "config.error.geolocation_range"
            ),
            ("/breakpoints/2/timezone", "config.error.unknown_timezone"),
            (
                "/breakpoints/2/cpuThrottling",
                "config.error.cpu_throttling_range"
            ),
        ]
    );
}

#[test]
fn an_empty_time_zone_is_the_engines_own_and_no_issue() {
    let fields = fields(json!({ "timezone": "", "geolocation": null }));
    assert!(check_extras(&fields, "/emulation").is_empty());
}

#[test]
fn the_config_check_covers_breakpoints_and_defaults() {
    let config: crate::ProjectConfig = serde_json::from_value(json!({
        "version": 1,
        "breakpoints": [{ "id": "a", "width": 400, "height": 800, "timezone": "Nowhere/City" }],
        "emulation": { "cpuThrottling": 500 },
    }))
    .expect("a config");
    let paths: Vec<String> = crate::check(&config)
        .into_iter()
        .map(|issue| issue.path)
        .collect();
    assert!(
        paths.contains(&"/breakpoints/0/timezone".to_owned()),
        "{paths:?}"
    );
    assert!(
        paths.contains(&"/emulation/cpuThrottling".to_owned()),
        "{paths:?}"
    );
}

#[test]
fn a_print_breakpoint_takes_its_size_from_the_paper() {
    let breakpoint: Breakpoint = serde_json::from_value(json!({
        "id": "print",
        "paper": "Letter",
        "media": { "mediaType": "print" },
    }))
    .expect("a breakpoint");
    assert_eq!(breakpoint.viewport(&[]), Some(Paper::Letter.viewport()));
    let config: crate::ProjectConfig = serde_json::from_value(json!({
        "version": 1,
        "breakpoints": [breakpoint],
    }))
    .expect("a config");
    assert!(crate::check(&config).is_empty(), "paper supplies the size");
}

#[test]
fn own_sides_win_over_the_paper() {
    let breakpoint: Breakpoint = serde_json::from_value(json!({
        "id": "print",
        "width": 700,
        "paper": "A4",
    }))
    .expect("a breakpoint");
    assert_eq!(
        breakpoint.viewport(&[]),
        Some(Viewport {
            width: 700,
            height: 1123
        })
    );
}
