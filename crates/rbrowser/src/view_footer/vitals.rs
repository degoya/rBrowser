//! The words of the vitals bar under every view (RB-090-71, `ui.viewFooter.vitals`): the Core
//! Web Vitals with the shape of their rating, a resource warning, every vital in words in the
//! tooltip and the vital whose element a click marks. Moved here from the label strip's piece of
//! RB-090-27; the host draws the bar below the breakpoint bar (`crate::host::footer`). Pure;
//! the catalogue gives every word.

use std::collections::BTreeMap;

use rb_engine::ViewId;
use rb_i18n::{t, t_with};
use rb_ipc::{Rating, Unavailable, ViewMetrics, VitalReading};
use rb_sync::VitalMetric;

use crate::cell_info::{Piece, PieceAction};

/// A Core Web Vital not measured yet in the bar: its name and a dash.
pub const MISSING_KEY: &str = "cells.info.vitalMissing";

/// The shape of each rating, in front of the value - never colour alone (Lighthouse's shapes:
/// circle good, square needs improvement, triangle poor).
pub const SHAPE_GOOD: &str = "\u{25cf}";
/// A square: needs improvement.
pub const SHAPE_NEEDS_IMPROVEMENT: &str = "\u{25a0}";
/// A triangle: poor, and a resource warning.
pub const SHAPE_POOR: &str = "\u{25b2}";

/// The vitals the bar itself names: the Core Web Vitals. FCP and TTFB are in the tooltip.
const IN_BAR: [VitalMetric; 3] = [VitalMetric::Lcp, VitalMetric::Cls, VitalMetric::Inp];

/// A vital's value as the bar shows it: ms below a second, seconds with two decimals above,
/// CLS with two decimals.
pub fn format_vital(metric: VitalMetric, value: f64) -> String {
    if metric == VitalMetric::Cls {
        return if value > 0.0 && value < 0.005 {
            "<0.01".to_owned()
        } else {
            format!("{value:.2}")
        };
    }
    if value < 1000.0 {
        t_with("cells.info.ms", &[("value", &format!("{}", value.round()))])
    } else {
        t_with(
            "cells.info.seconds",
            &[("value", &format!("{:.2}", value / 1000.0))],
        )
    }
}

/// The shape of `rating`.
pub fn shape_of(rating: Rating) -> &'static str {
    match rating {
        Rating::Good => SHAPE_GOOD,
        Rating::NeedsImprovement => SHAPE_NEEDS_IMPROVEMENT,
        Rating::Poor => SHAPE_POOR,
    }
}

fn rating_id(rating: Rating) -> &'static str {
    match rating {
        Rating::Good => "good",
        Rating::NeedsImprovement => "needs-improvement",
        Rating::Poor => "poor",
    }
}

fn unavailable_id(unavailable: Unavailable) -> &'static str {
    match unavailable {
        Unavailable::Pending => "pending",
        Unavailable::Unsupported => "unsupported",
        Unavailable::Soft => "soft",
        Unavailable::Engine => "engine",
    }
}

fn metric_name(metric: VitalMetric) -> String {
    t(&format!("shell.vitals.metric.{}", metric.id()))
}

/// Whether the engine can never measure `reading` (Firefox's CLS, WebKit's every vital): no
/// place in the bar, its reason in the tooltip.
fn never_measured(reading: &VitalReading) -> bool {
    matches!(
        reading.unavailable,
        Some(Unavailable::Engine | Unavailable::Unsupported)
    )
}

/// A reading with its value and rating.
fn measured(reading: &VitalReading) -> Option<(f64, Rating)> {
    Some((reading.value?, reading.rating?))
}

/// The vitals bar of every view of `metrics` that has one, by view.
pub fn bars(metrics: &[ViewMetrics]) -> BTreeMap<ViewId, Piece> {
    metrics
        .iter()
        .filter_map(|entry| Some((entry.view, bar(entry)?)))
        .collect()
}

/// The vitals bar of one view: the Core Web Vitals, each measured one with the shape of its
/// rating and its value, one not measured yet with a dash - so the bar keeps its place while a
/// page loads - one the engine never measures left out, and a resource warning; `None` without
/// a vital the view can measure (`vitals.enabled` off, a WebKit view) and without a warning, so
/// the view keeps that line.
pub fn bar(metrics: &ViewMetrics) -> Option<Piece> {
    let mut parts: Vec<String> = metrics
        .vitals
        .iter()
        .filter(|reading| IN_BAR.contains(&reading.metric))
        .filter_map(|reading| match measured(reading) {
            Some((value, rating)) => Some(t_with(
                "cells.info.vital",
                &[
                    ("shape", shape_of(rating)),
                    ("metric", &metric_name(reading.metric)),
                    ("value", &format_vital(reading.metric, value)),
                ],
            )),
            // A vital the engine never measures is left out: no dash that stays for good.
            None if never_measured(reading) => None,
            None => Some(t_with(
                MISSING_KEY,
                &[("metric", &metric_name(reading.metric))],
            )),
        })
        .collect();
    let resources = metrics.resources.as_ref();
    let cpu_warn = resources.is_some_and(|reading| reading.cpu_warn);
    let ram_warn = resources.is_some_and(|reading| reading.ram_warn);
    if cpu_warn {
        parts.push(t_with("cells.info.cpuHigh", &[("shape", SHAPE_POOR)]));
    }
    if ram_warn {
        parts.push(t_with("cells.info.memoryHigh", &[("shape", SHAPE_POOR)]));
    }
    if parts.is_empty() {
        return None;
    }
    let mut lines: Vec<String> = metrics.vitals.iter().map(vital_line).collect();
    if metrics.soft {
        lines.push(t("shell.vitals.soft"));
    }
    if cpu_warn {
        lines.push(t("shell.vitals.resources.cpuWarn"));
    }
    if ram_warn {
        lines.push(t("shell.vitals.resources.ramWarn"));
    }
    let action = match to_mark(metrics) {
        Some(metric) => {
            lines.push(t_with(
                "cells.info.mark",
                &[("metric", &metric_name(metric))],
            ));
            PieceAction::Mark(metric)
        }
        None => PieceAction::Activate,
    };
    Some(Piece {
        text: parts.join("  "),
        tooltip: lines.join("\n"),
        action,
    })
}

/// The vital of the bar whose element a click marks: the worst rated one with an element,
/// the first of them on a tie.
fn to_mark(metrics: &ViewMetrics) -> Option<VitalMetric> {
    let severity = |rating: Rating| match rating {
        Rating::Good => 0,
        Rating::NeedsImprovement => 1,
        Rating::Poor => 2,
    };
    metrics
        .vitals
        .iter()
        .filter(|reading| reading.highlight && IN_BAR.contains(&reading.metric))
        .filter_map(|reading| Some((reading.metric, severity(measured(reading)?.1))))
        .fold(
            None,
            |best: Option<(VitalMetric, u8)>, (metric, worse)| match best {
                Some((_, worst)) if worst >= worse => best,
                _ => Some((metric, worse)),
            },
        )
        .map(|(metric, _)| metric)
}

/// One vital in the tooltip: its value and rating in words, or why it has none.
fn vital_line(reading: &VitalReading) -> String {
    let metric = metric_name(reading.metric);
    match measured(reading) {
        Some((value, rating)) => t_with(
            "shell.vitals.badge",
            &[
                ("metric", &metric),
                ("value", &format_vital(reading.metric, value)),
                (
                    "rating",
                    &t(&format!("shell.vitals.rating.{}", rating_id(rating))),
                ),
            ],
        ),
        None => {
            let why = reading.unavailable.unwrap_or(Unavailable::Pending);
            t_with(
                "shell.vitals.missing",
                &[
                    ("metric", &metric),
                    (
                        "reason",
                        &t(&format!("shell.vitals.unavailable.{}", unavailable_id(why))),
                    ),
                ],
            )
        }
    }
}

#[cfg(test)]
#[path = "vitals_tests.rs"]
mod tests;
