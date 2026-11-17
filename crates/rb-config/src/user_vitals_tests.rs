use serde_json::json;

use super::*;
use crate::UserSettings;

#[test]
fn the_defaults_are_web_dev_thresholds_and_on() {
    let settings = UserSettings::default();
    assert!(settings.vitals.enabled);
    assert_eq!(settings.vitals.thresholds.lcp.good, 2500.0);
    assert_eq!(settings.vitals.thresholds.cls.poor, 0.25);
    assert_eq!(settings.vitals.thresholds.inp.poor, 500.0);
    assert!(settings.resources.enabled);
    assert_eq!(settings.resources.ram_warn_mb, 500);
    assert_eq!(settings.resources.cpu_warn_percent, 50);
}

#[test]
fn one_overridden_threshold_keeps_the_others() {
    let settings: UserSettings = serde_json::from_value(json!({
        "vitals": { "thresholds": { "cls": { "good": 0.05, "poor": 0.1 } } },
        "resources": { "ramWarnMb": 300 }
    }))
    .expect("parses");
    assert_eq!(
        settings.vitals.thresholds.cls,
        VitalThreshold {
            good: 0.05,
            poor: 0.1
        }
    );
    assert_eq!(settings.vitals.thresholds.lcp.good, 2500.0);
    assert_eq!(settings.resources.ram_warn_mb, 300);
    assert_eq!(settings.resources.cpu_warn_percent, 50);
}

#[test]
fn an_unknown_metric_is_refused() {
    let parsed =
        serde_json::from_value::<VitalThresholds>(json!({ "fid": { "good": 1, "poor": 2 } }));
    assert!(parsed.is_err());
}

#[test]
fn a_pair_rates_only_when_good_is_at_most_poor() {
    let ok = VitalThreshold {
        good: 1.0,
        poor: 1.0,
    };
    let upside_down = VitalThreshold {
        good: 2.0,
        poor: 1.0,
    };
    let negative = VitalThreshold {
        good: -1.0,
        poor: 1.0,
    };
    assert!(ok.is_valid());
    assert!(!upside_down.is_valid());
    assert!(!negative.is_valid());
}

#[test]
fn the_sections_round_trip() {
    let settings = UserSettings::default();
    let text = serde_json::to_value(&settings).expect("serializes");
    assert_eq!(
        text["vitals"]["thresholds"]["ttfb"],
        json!({ "good": 800.0, "poor": 1800.0 })
    );
    assert_eq!(text["resources"]["cpuWarnPercent"], json!(50));
}
