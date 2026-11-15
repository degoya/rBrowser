use rb_config::{VitalThreshold, VitalThresholds};
use rb_ipc::{Rating, Unavailable};
use rb_sync::{ClsValue, InpValue, LcpValue, VitalMetric, VitalsMessage};

use super::*;

fn report() -> VitalsMessage {
    VitalsMessage {
        v: 1,
        top: true,
        frame: None,
        segment: 0,
        soft: false,
        url: "http://a.test/".to_owned(),
        lcp: Some(LcpValue {
            value: 2600,
            element: Some("#hero".to_owned()),
        }),
        cls: Some(ClsValue {
            value: 0.3,
            sources: vec!["#banner".to_owned()],
        }),
        inp: None,
        fcp: Some(900),
        ttfb: None,
        unsupported: vec![VitalMetric::Ttfb],
    }
}

#[test]
fn web_dev_thresholds_rate_good_needs_improvement_and_poor() {
    let thresholds = VitalThresholds::default();
    assert_eq!(rate(VitalMetric::Lcp, 2500.0, &thresholds), Rating::Good);
    assert_eq!(
        rate(VitalMetric::Lcp, 2501.0, &thresholds),
        Rating::NeedsImprovement
    );
    assert_eq!(
        rate(VitalMetric::Lcp, 4000.0, &thresholds),
        Rating::NeedsImprovement
    );
    assert_eq!(rate(VitalMetric::Lcp, 4001.0, &thresholds), Rating::Poor);
    assert_eq!(rate(VitalMetric::Cls, 0.1, &thresholds), Rating::Good);
    assert_eq!(rate(VitalMetric::Cls, 0.26, &thresholds), Rating::Poor);
    assert_eq!(
        rate(VitalMetric::Inp, 300.0, &thresholds),
        Rating::NeedsImprovement
    );
    assert_eq!(rate(VitalMetric::Inp, 501.0, &thresholds), Rating::Poor);
    assert_eq!(rate(VitalMetric::Fcp, 1800.0, &thresholds), Rating::Good);
    assert_eq!(rate(VitalMetric::Ttfb, 1801.0, &thresholds), Rating::Poor);
}

#[test]
fn an_overridden_threshold_rates_with_the_new_values() {
    let thresholds = VitalThresholds {
        cls: VitalThreshold {
            good: 0.01,
            poor: 0.05,
        },
        ..VitalThresholds::default()
    };
    assert_eq!(rate(VitalMetric::Cls, 0.06, &thresholds), Rating::Poor);
}

#[test]
fn an_unusable_override_falls_back_to_web_dev() {
    let thresholds = VitalThresholds {
        lcp: VitalThreshold {
            good: 5000.0,
            poor: 100.0,
        },
        ..VitalThresholds::default()
    };
    assert_eq!(
        rate(VitalMetric::Lcp, 3000.0, &thresholds),
        Rating::NeedsImprovement
    );
}

#[test]
fn readings_rate_what_is_there_and_say_why_the_rest_is_not() {
    let (readings, soft) = readings(
        Some(&report()),
        Source::Reports,
        &VitalThresholds::default(),
    );
    assert!(!soft);
    let summary: Vec<_> = readings
        .iter()
        .map(|reading| {
            (
                reading.metric,
                reading.value,
                reading.rating,
                reading.unavailable,
                reading.highlight,
            )
        })
        .collect();
    assert_eq!(
        summary,
        [
            (
                VitalMetric::Lcp,
                Some(2600.0),
                Some(Rating::NeedsImprovement),
                None,
                true
            ),
            (VitalMetric::Cls, Some(0.3), Some(Rating::Poor), None, true),
            (
                VitalMetric::Inp,
                None,
                None,
                Some(Unavailable::Pending),
                false
            ),
            (
                VitalMetric::Fcp,
                Some(900.0),
                Some(Rating::Good),
                None,
                false
            ),
            (
                VitalMetric::Ttfb,
                None,
                None,
                Some(Unavailable::Unsupported),
                false
            ),
        ]
    );
}

#[test]
fn a_soft_segment_leaves_the_load_metrics_out() {
    let mut soft = report();
    soft.segment = 1;
    soft.soft = true;
    soft.lcp = None;
    soft.fcp = None;
    let (readings, is_soft) = readings(Some(&soft), Source::Reports, &VitalThresholds::default());
    assert!(is_soft);
    let lcp = readings.first().expect("lcp");
    assert_eq!(lcp.unavailable, Some(Unavailable::Soft));
    let cls = readings.get(1).expect("cls");
    assert_eq!(cls.rating, Some(Rating::Poor));
}

#[test]
fn a_clean_cls_has_nothing_to_mark() {
    let mut clean = report();
    clean.cls = Some(ClsValue {
        value: 0.0,
        sources: Vec::new(),
    });
    clean.inp = Some(InpValue {
        value: 80,
        target: Some("#buy".to_owned()),
        interactions: 2,
    });
    let (readings, _) = readings(Some(&clean), Source::Reports, &VitalThresholds::default());
    assert!(!readings.get(1).expect("cls").highlight);
    assert!(readings.get(2).expect("inp").highlight);
}

#[test]
fn without_a_report_an_engine_without_reports_is_named() {
    let (pending, _) = readings(None, Source::Reports, &VitalThresholds::default());
    assert!(
        pending
            .iter()
            .all(|reading| reading.unavailable == Some(Unavailable::Pending))
    );
    let (none, _) = readings(None, Source::None, &VitalThresholds::default());
    assert!(
        none.iter()
            .all(|reading| reading.unavailable == Some(Unavailable::Engine))
    );
    assert_eq!(none.len(), VitalMetric::ALL.len());
}

#[test]
fn the_selector_behind_a_metric_is_the_reports() {
    let report = report();
    assert_eq!(selector_of(&report, VitalMetric::Lcp), Some("#hero"));
    assert_eq!(selector_of(&report, VitalMetric::Cls), Some("#banner"));
    assert_eq!(selector_of(&report, VitalMetric::Inp), None);
    assert_eq!(selector_of(&report, VitalMetric::Fcp), None);
}
