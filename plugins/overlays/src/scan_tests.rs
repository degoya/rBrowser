use rb_plugin_api::{Severity, ViewInfo};
use serde_json::json;

use super::*;

fn view() -> ViewInfo {
    ViewInfo {
        id: 3,
        label: "Mobile".to_owned(),
        breakpoint: "sm".to_owned(),
        engine: "chromium".to_owned(),
        url: "http://localhost/overlays.html".to_owned(),
        ..ViewInfo::default()
    }
}

fn hit(selector: &str) -> serde_json::Value {
    json!({
        "selector": selector,
        "path": "html > body > button:nth-of-type(1)",
        "label": "20x20",
        "rect": { "x": 1, "y": 2, "width": 20, "height": 20 },
    })
}

#[test]
fn the_scan_overlays_are_the_ones_that_find_elements() {
    for id in [
        "stacking",
        "scrollbars",
        "text-spacing",
        "target-size",
        "small-text",
    ] {
        assert!(is_scan(id), "{id}");
    }
    for id in ["overflow", "focus-tracking", "css-off", "js-off"] {
        assert!(!is_scan(id), "{id}");
    }
    assert_eq!(rule("target-size"), Some("target-size"));
    assert_eq!(rule("small-text"), Some("small-text"));
    assert_eq!(rule("text-spacing"), Some("text-spacing-clip"));
    assert_eq!(rule("scrollbars"), Some("unneeded-scrollbar"));
    assert_eq!(rule("stacking"), None);
}

#[test]
fn the_script_runs_the_bundled_scan_with_the_overlay_and_its_options() {
    let script = script("target-size", &json!({ "sizePx": 24 }));
    assert!(script.contains(r#"__rbOverlaysMeasure.scan("target-size", {"sizePx":24})"#));
    assert!(script.starts_with("(() => {"));
}

#[test]
fn a_scan_is_parsed_from_an_object_or_its_text_and_bounded() {
    let items: Vec<_> = (0..MAX_ITEMS + 10)
        .map(|n| hit(&format!(".b{n}")))
        .collect();
    let answer = json!({ "count": MAX_ITEMS + 10, "items": items }).to_string();
    let scan = Scan::parse(&answer).expect("parsed");
    assert_eq!(scan.count, (MAX_ITEMS + 10) as u64);
    assert_eq!(scan.items.len(), MAX_ITEMS);
    let text =
        serde_json::to_string(&json!({ "count": 1, "items": [hit(".a\u{7}b")] }).to_string())
            .expect("JSON");
    let scan = Scan::parse(&text).expect("parsed from text");
    assert_eq!(scan.items[0].selector, ".ab");
    assert!(Scan::parse("null").is_none());
    assert!(Scan::parse("[1]").is_none());
}

#[test]
fn hits_become_findings_of_the_rule_with_view_url_and_path() {
    let scan = Scan::parse(&json!({ "count": 1, "items": [hit("button.a")] }).to_string())
        .expect("parsed");
    let found = findings("target-size", &view(), &scan);
    assert_eq!(found.len(), 1);
    let finding = &found[0];
    assert_eq!(finding.rule, "target-size");
    assert_eq!(finding.severity, Severity::Warning);
    assert_eq!(finding.message, "plugin.overlays.findings.target_size");
    assert_eq!(
        finding.params,
        vec![("label".to_owned(), "20x20".to_owned())]
    );
    assert_eq!(finding.view, Some(3));
    assert_eq!(
        finding.url.as_deref(),
        Some("http://localhost/overlays.html")
    );
    assert_eq!(finding.selector.as_deref(), Some("button.a"));
    assert_eq!(
        finding.path.as_deref(),
        Some("html > body > button:nth-of-type(1)")
    );
    assert!(findings("stacking", &view(), &scan).is_empty());
}

#[test]
fn the_answer_keeps_the_count_and_the_first_items() {
    let items: Vec<_> = (0..80).map(|n| hit(&format!(".b{n}"))).collect();
    let scan = Scan::parse(&json!({ "count": 80, "items": items }).to_string()).expect("parsed");
    let answered = scan.answer(&view(), usize::MAX);
    assert_eq!(answered["view"], json!(3));
    assert_eq!(answered["count"], json!(80));
    assert_eq!(answered["shown"], json!(MAX_ANSWERED));
    assert_eq!(
        answered["items"].as_array().map(Vec::len),
        Some(MAX_ANSWERED)
    );
    let fitted = scan.answer(&view(), 7);
    assert_eq!(fitted["count"], json!(80));
    assert_eq!(fitted["shown"], json!(7));
    assert_eq!(fitted["items"].as_array().map(Vec::len), Some(7));
}
