use serde_json::json;

use super::*;

fn narrow() -> serde_json::Value {
    json!({
        "hasHorizontalScroll": true,
        "scrollWidth": 416,
        "clientWidth": 375,
        "innerWidth": 375,
        "causes": [{ "selector": "div.promo", "rect": { "x": 16, "y": 120, "width": 400, "height": 80 }, "overhang": 41 }],
        "vw100": [{ "selector": "section.hero", "rect": { "x": 0, "y": 0, "width": 375, "height": 200 } }]
    })
}

#[test]
fn the_script_is_one_expression_around_the_bundle() {
    let script = measure_script(false);
    assert!(script.starts_with("(() => {"));
    assert!(script.ends_with("})()"));
    assert!(script.contains("__rbOverlaysMeasure"));
    assert!(script.contains("measure({ show100vw: false })"));
}

#[test]
fn a_report_parses_from_an_object_or_from_json_text() {
    let report = Report::parse(&narrow().to_string()).expect("object");
    assert!(report.has_horizontal_scroll);
    assert_eq!(report.causes[0].selector, "div.promo");
    assert_eq!(report.causes[0].overhang, Some(41));
    assert_eq!(report.vw100[0].overhang, None);
    let text = serde_json::Value::String(narrow().to_string()).to_string();
    assert_eq!(Report::parse(&text), Some(report));
    assert_eq!(Report::parse("null"), None);
    assert_eq!(Report::parse("[1]"), None);
}

#[test]
fn a_report_is_bounded() {
    let mark = json!({ "selector": format!("a\u{7}{}", "b".repeat(400)), "overhang": 1 });
    let report = Report::parse(
        &json!({ "hasHorizontalScroll": true, "causes": vec![mark; 80] }).to_string(),
    )
    .expect("parses");
    assert_eq!(report.causes.len(), MAX_MARKS);
    assert_eq!(
        report.causes[0].selector.chars().count(),
        MAX_SELECTOR_CHARS
    );
    assert!(!report.causes[0].selector.contains('\u{7}'));
}

#[test]
fn the_log_line_names_the_causes_and_their_overhang() {
    let report = Report::parse(&narrow().to_string()).expect("parses");
    assert_eq!(
        report.log_line(3).as_deref(),
        Some(
            "view 3: horizontal scroll, scrollWidth 416 > clientWidth 375; caused by div.promo (+41 px)"
        )
    );
    assert_eq!(Report::default().log_line(3), None);
}

#[test]
fn the_log_line_counts_causes_beyond_five() {
    let mark = json!({ "selector": ".x", "overhang": 2 });
    let report =
        Report::parse(&json!({ "hasHorizontalScroll": true, "causes": vec![mark; 7] }).to_string())
            .expect("parses");
    let line = report.log_line(1).expect("a line");
    assert!(line.ends_with(" and 2 more"), "{line}");
}
