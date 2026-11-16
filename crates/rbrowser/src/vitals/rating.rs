//! The rating of the Web Vitals (RB-060-10): web.dev's scheme - at most `good` is good, above
//! `poor` is poor, between them needs improvement - with the thresholds of `vitals.thresholds`;
//! a pair that cannot rate (good above poor, negative) falls back to web.dev's. And the readings
//! a cell label shows: every metric in label order, rated, or the reason it has no value.

use rb_config::{VitalThreshold, VitalThresholds};
use rb_ipc::{Rating, Unavailable, VitalReading};
use rb_sync::{VitalMetric, VitalsMessage};

/// Where a view's vitals come from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// The view's agent reports them (Chromium, Firefox).
    Reports,
    /// Its engine reports none (WebKit through the sidecar).
    None,
}

fn pair(metric: VitalMetric, thresholds: &VitalThresholds) -> VitalThreshold {
    let pick = |thresholds: &VitalThresholds| match metric {
        VitalMetric::Lcp => thresholds.lcp,
        VitalMetric::Cls => thresholds.cls,
        VitalMetric::Inp => thresholds.inp,
        VitalMetric::Fcp => thresholds.fcp,
        VitalMetric::Ttfb => thresholds.ttfb,
    };
    let chosen = pick(thresholds);
    if chosen.is_valid() {
        chosen
    } else {
        pick(&VitalThresholds::default())
    }
}

/// `value`'s rating for `metric`.
#[must_use]
pub fn rate(metric: VitalMetric, value: f64, thresholds: &VitalThresholds) -> Rating {
    let pair = pair(metric, thresholds);
    if value <= pair.good {
        Rating::Good
    } else if value <= pair.poor {
        Rating::NeedsImprovement
    } else {
        Rating::Poor
    }
}

/// The value of `metric` in `report`, and whether there is an element to mark.
fn value_of(report: &VitalsMessage, metric: VitalMetric) -> Option<(f64, bool)> {
    match metric {
        VitalMetric::Lcp => report
            .lcp
            .as_ref()
            .map(|lcp| (f64::from(lcp.value), lcp.element.is_some())),
        VitalMetric::Cls => report
            .cls
            .as_ref()
            .map(|cls| (cls.value, !cls.sources.is_empty())),
        VitalMetric::Inp => report
            .inp
            .as_ref()
            .map(|inp| (f64::from(inp.value), inp.target.is_some())),
        VitalMetric::Fcp => report.fcp.map(|fcp| (f64::from(fcp), false)),
        VitalMetric::Ttfb => report.ttfb.map(|ttfb| (f64::from(ttfb), false)),
    }
}

/// The metrics a soft navigation's segment does not measure.
fn load_metric(metric: VitalMetric) -> bool {
    matches!(
        metric,
        VitalMetric::Lcp | VitalMetric::Fcp | VitalMetric::Ttfb
    )
}

fn missing(metric: VitalMetric, unavailable: Unavailable) -> VitalReading {
    VitalReading {
        metric,
        value: None,
        rating: None,
        unavailable: Some(unavailable),
        highlight: false,
    }
}

/// Every metric of a view in label order, and whether they are of a soft navigation.
#[must_use]
pub fn readings(
    report: Option<&VitalsMessage>,
    source: Source,
    thresholds: &VitalThresholds,
) -> (Vec<VitalReading>, bool) {
    let Some(report) = report else {
        let reason = match source {
            Source::Reports => Unavailable::Pending,
            Source::None => Unavailable::Engine,
        };
        return (
            VitalMetric::ALL
                .iter()
                .map(|metric| missing(*metric, reason))
                .collect(),
            false,
        );
    };
    let readings = VitalMetric::ALL
        .iter()
        .map(|&metric| {
            if report.unsupported.contains(&metric) {
                return missing(metric, Unavailable::Unsupported);
            }
            match value_of(report, metric) {
                Some((value, highlight)) => VitalReading {
                    metric,
                    value: Some(value),
                    rating: Some(rate(metric, value, thresholds)),
                    unavailable: None,
                    highlight,
                },
                None if report.soft && load_metric(metric) => missing(metric, Unavailable::Soft),
                None => missing(metric, Unavailable::Pending),
            }
        })
        .collect();
    (readings, report.soft)
}

/// The selector of the element behind `metric` in `report`: the LCP element, the largest
/// shift source, the interaction's target; `None` for the other metrics.
#[must_use]
pub fn selector_of(report: &VitalsMessage, metric: VitalMetric) -> Option<&str> {
    match metric {
        VitalMetric::Lcp => report.lcp.as_ref()?.element.as_deref(),
        VitalMetric::Cls => report.cls.as_ref()?.sources.first().map(String::as_str),
        VitalMetric::Inp => report.inp.as_ref()?.target.as_deref(),
        VitalMetric::Fcp | VitalMetric::Ttfb => None,
    }
}

#[cfg(test)]
#[path = "rating_tests.rs"]
mod tests;
