use serde_json::json;

use super::*;
use crate::{AgentMessage, parse_message};

fn full() -> serde_json::Value {
    json!({
        "kind": "vitals", "v": 1, "top": true, "segment": 0, "soft": false,
        "url": "http://localhost:5173/",
        "lcp": { "value": 1840, "element": "#hero > img" },
        "cls": { "value": 0.1834, "sources": ["#banner"] },
        "inp": { "value": 312, "target": "#buy", "interactions": 4 },
        "fcp": 412, "ttfb": 96, "unsupported": []
    })
}

fn parsed(value: &serde_json::Value) -> VitalsMessage {
    match parse_message(&value.to_string()).expect("parses") {
        AgentMessage::Vitals(vitals) => vitals,
        other => panic!("not vitals: {other:?}"),
    }
}

#[test]
fn a_full_report_parses_and_checks() {
    let vitals = parsed(&full());
    assert_eq!(vitals.check(), Ok(()));
    assert_eq!(vitals.lcp.as_ref().map(|lcp| lcp.value), Some(1840));
    assert_eq!(vitals.inp.as_ref().map(|inp| inp.interactions), Some(4));
    assert!(vitals.unsupported.is_empty());
}

#[test]
fn a_negative_or_fractional_time_is_not_a_report() {
    for bad in [json!(-1), json!(412.5)] {
        let mut value = full();
        value["fcp"] = bad;
        assert!(parse_message(&value.to_string()).is_err());
    }
}

#[test]
fn unsupported_metrics_are_named() {
    let mut value = full();
    value["unsupported"] = json!(["cls", "inp"]);
    let vitals = parsed(&value);
    assert_eq!(vitals.unsupported, vec![VitalMetric::Cls, VitalMetric::Inp]);
}

#[test]
fn an_unknown_metric_name_is_refused() {
    let mut value = full();
    value["unsupported"] = json!(["fid"]);
    assert!(parse_message(&value.to_string()).is_err());
}

#[test]
fn absurd_values_fail_the_check() {
    for (path, bad) in [
        ("ttfb", json!(MAX_TIME_MS + 1)),
        ("cls", json!({ "value": 1.0e6, "sources": [] })),
        ("cls", json!({ "value": -0.5, "sources": [] })),
    ] {
        let mut value = full();
        value[path] = bad;
        assert_eq!(parsed(&value).check(), Err(VitalsError::Value), "{path}");
    }
}

#[test]
fn long_or_many_selectors_fail_the_check() {
    let mut value = full();
    value["lcp"]["element"] = json!("a".repeat(MAX_SELECTOR_CHARS + 1));
    assert_eq!(parsed(&value).check(), Err(VitalsError::Selector));
    let mut value = full();
    value["cls"]["sources"] = json!(vec!["#a"; MAX_SOURCES + 1]);
    assert_eq!(parsed(&value).check(), Err(VitalsError::Selector));
    let mut value = full();
    value["inp"]["target"] = json!("#a\nb");
    assert_eq!(parsed(&value).check(), Err(VitalsError::Selector));
}

#[test]
fn a_long_url_fails_the_check() {
    let mut value = full();
    value["url"] = json!(format!("http://a.test/{}", "x".repeat(MAX_URL_CHARS)));
    assert_eq!(parsed(&value).check(), Err(VitalsError::Url));
}

#[test]
fn the_metric_names_are_stable() {
    let names: Vec<&str> = VitalMetric::ALL.iter().map(|metric| metric.id()).collect();
    assert_eq!(names, ["lcp", "cls", "inp", "fcp", "ttfb"]);
}
