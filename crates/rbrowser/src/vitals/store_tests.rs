use std::time::Instant;

use rb_config::{EngineKind, UserSettings};
use rb_engine::ViewId;
use rb_ipc::{Rating, Unavailable, ViewThrottle};
use rb_sync::{ClsValue, VitalsMessage};

use super::*;

fn options() -> Options {
    Options::of(&UserSettings::default())
}

fn facts(view: u32, engine: EngineKind) -> ViewFacts {
    ViewFacts {
        view: ViewId(view),
        engine,
        throttle: None,
    }
}

fn report(cls: f64) -> VitalsMessage {
    VitalsMessage {
        v: 1,
        top: true,
        frame: None,
        segment: 0,
        soft: false,
        url: "http://a.test/".to_owned(),
        lcp: None,
        cls: Some(ClsValue {
            value: cls,
            sources: vec!["#banner".to_owned()],
        }),
        inp: None,
        fcp: None,
        ttfb: None,
        unsupported: Vec::new(),
    }
}

fn heap(mb: f64) -> PageMetrics {
    PageMetrics {
        timestamp_s: 1.0,
        task_duration_s: 0.0,
        js_heap_bytes: mb * 1_048_576.0,
    }
}

#[test]
fn a_reported_view_shows_its_rated_vitals_and_its_resources() {
    let mut store = Store::default();
    store.on_report(ViewId(1), report(0.3));
    let limits = options().limits;
    store.on_sample(ViewId(1), heap(64.0), Instant::now(), limits);
    let metrics = store.metrics(&[facts(1, EngineKind::Chromium)], &options());
    let view = metrics.first().expect("one view");
    let cls = view.vitals.get(1).expect("cls");
    assert_eq!(cls.rating, Some(Rating::Poor));
    let resources = view.resources.as_ref().expect("resources");
    assert_eq!(resources.memory_mb, Some(64.0));
}

#[test]
fn a_view_without_a_report_or_sample_is_pending() {
    let store = Store::default();
    let metrics = store.metrics(&[facts(1, EngineKind::Chromium)], &options());
    let view = metrics.first().expect("one view");
    assert!(
        view.vitals
            .iter()
            .all(|reading| reading.unavailable == Some(Unavailable::Pending))
    );
    assert_eq!(
        view.resources.as_ref().and_then(|r| r.unavailable),
        Some(Unavailable::Pending)
    );
}

#[test]
fn external_engines_say_what_they_cannot_give() {
    let store = Store::default();
    let metrics = store.metrics(
        &[facts(1, EngineKind::Firefox), facts(2, EngineKind::Webkit)],
        &options(),
    );
    let firefox = metrics.first().expect("firefox");
    let webkit = metrics.get(1).expect("webkit");
    assert_eq!(
        firefox.vitals.first().and_then(|r| r.unavailable),
        Some(Unavailable::Pending)
    );
    assert_eq!(
        webkit.vitals.first().and_then(|r| r.unavailable),
        Some(Unavailable::Engine)
    );
    for view in [firefox, webkit] {
        assert_eq!(
            view.resources.as_ref().and_then(|r| r.unavailable),
            Some(Unavailable::Engine)
        );
    }
}

#[test]
fn switched_off_parts_are_left_out() {
    let mut store = Store::default();
    store.on_report(ViewId(1), report(0.3));
    let off = Options {
        vitals: false,
        resources: false,
        ..options()
    };
    let mut view = facts(1, EngineKind::Chromium);
    view.throttle = Some(ViewThrottle::Hidden);
    let metrics = store.metrics(&[view], &off);
    let only = metrics.first().expect("one view");
    assert!(only.vitals.is_empty());
    assert!(only.resources.is_none());
    assert_eq!(only.throttle, Some(ViewThrottle::Hidden));
}

#[test]
fn a_new_document_drops_the_old_report_and_closed_views_are_forgotten() {
    let mut store = Store::default();
    store.on_report(ViewId(1), report(0.3));
    store.on_report(ViewId(2), report(0.0));
    store.on_document(ViewId(1));
    assert!(store.report(ViewId(1)).is_none());
    store.retain(&[ViewId(1)]);
    assert!(store.report(ViewId(2)).is_none());
}

#[test]
fn memory_over_the_limit_is_a_change_and_a_warning() {
    let mut store = Store::default();
    let limits = options().limits;
    let changes = store.on_sample(ViewId(1), heap(700.0), Instant::now(), limits);
    assert_eq!(changes, vec![Change::RamWarn(700.0)]);
    let metrics = store.metrics(&[facts(1, EngineKind::Chromium)], &options());
    let resources = metrics
        .first()
        .and_then(|view| view.resources.clone())
        .expect("resources");
    assert!(resources.ram_warn);
    store.clear_resources();
    let metrics = store.metrics(&[facts(1, EngineKind::Chromium)], &options());
    assert_eq!(
        metrics
            .first()
            .and_then(|view| view.resources.as_ref())
            .and_then(|r| r.unavailable),
        Some(Unavailable::Pending)
    );
}
