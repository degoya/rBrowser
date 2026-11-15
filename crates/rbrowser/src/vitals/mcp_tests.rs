use std::time::Instant;

use rb_config::{EngineKind, UserSettings};
use rb_engine::ViewId;
use rb_ipc::ViewThrottle;
use rb_sync::{ClsValue, LcpValue, VitalMetric, VitalsMessage};
use serde_json::json;

use super::*;
use crate::vitals::resources::PageMetrics;
use crate::vitals::store::{Options, Store};

fn row(view: u32, engine: EngineKind) -> ViewRow {
    ViewRow {
        view: ViewId(view),
        breakpoint: Some("mobile".to_owned()),
        engine,
        url: "http://a.test/".to_owned(),
        throttle: None,
    }
}

fn store() -> Store {
    let mut store = Store::default();
    store.on_report(
        ViewId(1),
        VitalsMessage {
            v: 1,
            top: true,
            frame: None,
            segment: 0,
            soft: false,
            url: "http://a.test/".to_owned(),
            lcp: Some(LcpValue {
                value: 1200,
                element: Some("#hero".to_owned()),
            }),
            cls: Some(ClsValue {
                value: 0.31,
                sources: vec!["#banner".to_owned()],
            }),
            inp: None,
            fcp: None,
            ttfb: None,
            unsupported: vec![VitalMetric::Ttfb],
        },
    );
    let limits = Options::of(&UserSettings::default()).limits;
    store.on_sample(
        ViewId(1),
        PageMetrics {
            timestamp_s: 1.0,
            task_duration_s: 0.0,
            js_heap_bytes: 32.0 * 1_048_576.0,
        },
        Instant::now(),
        limits,
    );
    store
}

#[test]
fn every_view_has_values_or_na_with_a_reason() {
    let options = Options::of(&UserSettings::default());
    let mut hidden = row(3, EngineKind::Webkit);
    hidden.throttle = Some(ViewThrottle::Hidden);
    let answer = answer(&store(), &[row(1, EngineKind::Chromium), hidden], &options);
    let first = &answer["views"][0];
    assert_eq!(first["view"], json!(1));
    assert_eq!(
        first["vitals"]["lcp"],
        json!({ "value": 1200.0, "unit": "ms", "rating": "good", "element": "#hero" })
    );
    assert_eq!(first["vitals"]["cls"]["rating"], json!("poor"));
    assert_eq!(first["vitals"]["cls"]["sources"], json!(["#banner"]));
    assert_eq!(first["vitals"]["inp"]["na"], json!("pending"));
    assert_eq!(first["vitals"]["ttfb"]["na"], json!("unsupported"));
    assert!(first["vitals"]["ttfb"]["reason"].is_string());
    assert_eq!(first["resources"]["memoryMb"], json!(32.0));
    assert_eq!(first["throttle"], json!({ "state": "visible" }));
    let webkit = &answer["views"][1];
    assert_eq!(webkit["vitals"]["lcp"]["na"], json!("engine"));
    assert_eq!(webkit["resources"]["na"], json!("engine"));
    assert_eq!(webkit["throttle"], json!({ "state": "hidden" }));
}

#[test]
fn switched_off_parts_are_na_naming_the_setting() {
    let options = Options {
        vitals: false,
        resources: false,
        ..Options::of(&UserSettings::default())
    };
    let answer = answer(&store(), &[row(1, EngineKind::Chromium)], &options);
    let view = &answer["views"][0];
    assert_eq!(view["vitals"]["lcp"]["na"], json!("off"));
    assert_eq!(
        view["vitals"]["lcp"]["settingsKey"],
        json!("vitals.enabled")
    );
    assert_eq!(view["resources"]["na"], json!("off"));
    assert_eq!(view["resources"]["settingsKey"], json!("resources.enabled"));
}

#[test]
fn the_view_list_argument_is_checked() {
    assert_eq!(
        wanted(&json!({ "views": [1, 3] })),
        Ok(Some(vec![ViewId(1), ViewId(3)]))
    );
    assert_eq!(wanted(&json!({})), Ok(None));
    assert!(wanted(&json!({ "views": ["x"] })).is_err());
}
