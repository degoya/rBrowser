// RB-090-71: the vitals bar's words, moved with the code from the label strip's piece of
// RB-090-27 (`cell_info/text_tests.rs` before).

use serde_json::json;

use super::*;

fn metrics(value: serde_json::Value) -> ViewMetrics {
    serde_json::from_value(value).expect("metrics")
}

#[test]
fn a_value_is_in_ms_below_a_second_and_in_seconds_above() {
    assert_eq!(format_vital(VitalMetric::Inp, 120.4), "120 ms");
    assert_eq!(format_vital(VitalMetric::Lcp, 1840.0), "1.84 s");
    assert_eq!(format_vital(VitalMetric::Cls, 0.123), "0.12");
    assert_eq!(format_vital(VitalMetric::Cls, 0.001), "<0.01");
    assert_eq!(format_vital(VitalMetric::Cls, 0.0), "0.00");
}

#[test]
fn the_bar_names_the_core_vitals_with_their_shapes_and_the_tooltip_all_in_words() {
    let piece = bar(&metrics(json!({
        "view": 1,
        "vitals": [
            { "metric": "lcp", "value": 1840.0, "rating": "good" },
            { "metric": "cls", "value": 0.3, "rating": "poor" },
            { "metric": "inp", "unavailable": "pending" },
            { "metric": "fcp", "value": 900.0, "rating": "good" },
            { "metric": "ttfb", "value": 80.0, "rating": "good" }
        ]
    })))
    .expect("a bar");
    assert_eq!(
        piece.text,
        format!("{SHAPE_GOOD} LCP 1.84 s  {SHAPE_POOR} CLS 0.30  INP \u{2013}"),
        "LCP, CLS and INP only; INP is not measured yet: a dash"
    );
    let lines: Vec<&str> = piece.tooltip.lines().collect();
    assert_eq!(lines.len(), 5, "every vital: {lines:?}");
    assert_eq!(lines[0], "LCP 1.84 s: good");
    assert_eq!(lines[1], "CLS 0.30: poor");
    assert_eq!(lines[2], "INP: not measured yet");
    assert_eq!(lines[4], "TTFB 80 ms: good");
    assert_eq!(
        piece.action,
        PieceAction::Activate,
        "no element to mark: the click activates the view"
    );
}

#[test]
fn a_click_marks_the_element_of_the_worst_rated_vital_in_the_bar() {
    let piece = bar(&metrics(json!({
        "view": 1,
        "vitals": [
            { "metric": "lcp", "value": 3000.0, "rating": "needs-improvement", "highlight": true },
            { "metric": "cls", "value": 0.3, "rating": "poor", "highlight": true },
            { "metric": "inp", "value": 900.0, "rating": "poor", "highlight": true },
            { "metric": "fcp", "value": 9000.0, "rating": "poor", "highlight": true }
        ]
    })))
    .expect("a bar");
    assert_eq!(
        piece.action,
        PieceAction::Mark(VitalMetric::Cls),
        "the first of the poor ones; FCP is not in the bar"
    );
    assert!(
        piece
            .tooltip
            .ends_with("Click: mark the element behind CLS in the view."),
        "{}",
        piece.tooltip
    );
}

#[test]
fn a_rating_is_a_shape_never_colour_alone() {
    assert_eq!(shape_of(Rating::Good), SHAPE_GOOD);
    assert_eq!(shape_of(Rating::NeedsImprovement), SHAPE_NEEDS_IMPROVEMENT);
    assert_eq!(shape_of(Rating::Poor), SHAPE_POOR);
    assert_ne!(SHAPE_GOOD, SHAPE_NEEDS_IMPROVEMENT);
    assert_ne!(SHAPE_NEEDS_IMPROVEMENT, SHAPE_POOR);
}

#[test]
fn nothing_measured_yet_keeps_the_bar_with_dashes() {
    // The bar keeps its place while the page loads: no height that comes and goes.
    let pending = metrics(json!({
        "view": 1,
        "vitals": [
            { "metric": "lcp", "unavailable": "pending" },
            { "metric": "cls", "unavailable": "pending" }
        ],
        "resources": { "cpuPercent": 12.0, "memoryMb": 48.0 }
    }));
    let piece = bar(&pending).expect("a bar");
    assert_eq!(piece.text, "LCP \u{2013}  CLS \u{2013}");
    assert_eq!(piece.action, PieceAction::Activate);
}

#[test]
fn without_vitals_and_without_a_warning_there_is_no_bar() {
    let off = metrics(json!({
        "view": 1,
        "resources": { "cpuPercent": 12.0, "memoryMb": 48.0 }
    }));
    assert_eq!(bar(&off), None);
}

#[test]
fn a_resource_warning_shows_even_without_vitals() {
    let piece = bar(&metrics(json!({
        "view": 1,
        "resources": { "cpuPercent": 95.0, "memoryMb": 900.0, "cpuWarn": true, "ramWarn": true }
    })))
    .expect("a warning");
    assert_eq!(
        piece.text,
        format!("{SHAPE_POOR} CPU high  {SHAPE_POOR} Memory high")
    );
    assert!(piece.tooltip.contains("five seconds"), "{}", piece.tooltip);
    assert!(
        piece.tooltip.contains("memory is above"),
        "{}",
        piece.tooltip
    );
}

#[test]
fn every_view_with_vitals_gets_its_bar() {
    let all = [
        metrics(
            json!({ "view": 1, "vitals": [{ "metric": "lcp", "value": 500.0, "rating": "good" }] }),
        ),
        metrics(json!({ "view": 2 })),
        metrics(json!({ "view": 3, "vitals": [{ "metric": "inp", "unavailable": "engine" }] })),
    ];
    let bars = bars(&all);
    assert_eq!(
        bars.keys().copied().collect::<Vec<_>>(),
        [ViewId(1)],
        "view 3 can never measure INP: no bar"
    );
    assert_eq!(bars[&ViewId(1)].text, format!("{SHAPE_GOOD} LCP 500 ms"));
}

#[test]
fn a_vital_the_engine_can_never_measure_is_left_out_of_the_bar() {
    // Firefox measures no CLS: no dash that would stay for good; the tooltip still says why.
    let firefox = bar(&metrics(json!({
        "view": 2,
        "vitals": [
            { "metric": "lcp", "value": 900.0, "rating": "good" },
            { "metric": "cls", "unavailable": "engine" },
            { "metric": "inp", "unavailable": "pending" }
        ]
    })))
    .expect("a bar");
    assert_eq!(
        firefox.text,
        format!("{SHAPE_GOOD} LCP 900 ms  INP \u{2013}")
    );
    assert!(
        firefox.tooltip.contains("CLS: n/a in this engine"),
        "{}",
        firefox.tooltip
    );
}

#[test]
fn a_view_that_can_never_measure_has_no_bar() {
    // WebKit measures nothing: no bar takes a line from the view for good.
    let webkit = metrics(json!({
        "view": 3,
        "vitals": [
            { "metric": "lcp", "unavailable": "engine" },
            { "metric": "cls", "unavailable": "unsupported" },
            { "metric": "inp", "unavailable": "engine" }
        ]
    }));
    assert_eq!(bar(&webkit), None);
    // A warning still shows.
    let warned = metrics(json!({
        "view": 3,
        "vitals": [{ "metric": "lcp", "unavailable": "engine" }],
        "resources": { "cpuPercent": 95.0, "memoryMb": 900.0, "cpuWarn": true }
    }));
    assert_eq!(
        bar(&warned).map(|piece| piece.text),
        Some(format!("{SHAPE_POOR} CPU high"))
    );
}
