use serde_json::json;

use super::*;

static HIDDEN: HiddenTargets =
    HiddenTargets::new(&["devtools:", "rbrowser:"], &["about:blank#marker"]);

fn hidden() -> HiddenTargets {
    HiddenTargets::new(&["devtools:", "rbrowser:"], &["about:blank#marker"])
}

#[test]
fn internal_schemes_and_marker_urls_are_hidden_in_any_case() {
    let filter = hidden();
    assert!(filter.is_hidden("T1", "rbrowser://ui/index.html"));
    assert!(filter.is_hidden("T2", "RBrowser://ui/settings.html"));
    assert!(filter.is_hidden("T3", "devtools://devtools/bundled/inspector.html"));
    assert!(filter.is_hidden("T4", "about:blank#marker"));
    assert!(!filter.is_hidden("T5", "https://site.test/"));
    assert!(!filter.is_hidden("T6", "about:blank"));
    assert!(!filter.is_hidden("T7", "rbrowse"));
}

#[test]
fn a_registered_id_is_hidden_whatever_it_shows_until_forgotten() {
    let filter = hidden();
    filter.register("SHELL");
    assert!(filter.is_hidden("SHELL", "https://site.test/"));
    assert!(filter.is_hidden("SHELL", ""));
    filter.forget("SHELL");
    assert!(!filter.is_hidden("SHELL", "https://site.test/"));
}

#[test]
fn a_static_registry_works_across_calls() {
    HIDDEN.register("STATIC");
    assert!(HIDDEN.is_hidden("STATIC", ""));
}

#[test]
fn target_infos_are_judged_by_id_and_url() {
    let filter = hidden();
    assert!(hides_info(
        &filter,
        &json!({"targetId": "A", "url": "rbrowser://ui/"})
    ));
    assert!(!hides_info(
        &filter,
        &json!({"targetId": "B", "url": "https://a.test/"})
    ));
    // Without an id or URL nothing is known: shown, like Chromium would.
    assert!(!hides_info(&filter, &json!({})));
}

#[test]
fn visible_infos_drop_the_hidden_ones_in_order() {
    let filter = hidden();
    let infos = vec![
        json!({"targetId": "SHELL", "type": "page", "url": "rbrowser://ui/index.html"}),
        json!({"targetId": "V1", "type": "page", "url": "https://a.test/"}),
        json!({"targetId": "DT", "type": "page", "url": "devtools://devtools/x"}),
        json!({"targetId": "V2", "type": "page", "url": "https://b.test/"}),
    ];
    let visible = visible_infos(&filter, &infos);
    let shown: Vec<&str> = visible
        .iter()
        .filter_map(|info| info["targetId"].as_str())
        .collect();
    assert_eq!(shown, ["V1", "V2"]);
}

/// Hides every page but `PAGE`, whatever its id says alone.
struct OnePage;

impl TargetFilter for OnePage {
    fn is_hidden(&self, _target_id: &str, _url: &str) -> bool {
        false
    }

    fn hides_info(&self, info: &Value) -> bool {
        info["type"] == "page" && info["targetId"] != "PAGE"
    }
}

#[test]
fn a_filter_may_judge_target_infos_by_their_type() {
    let popup = json!({ "targetId": "POPUP", "type": "page", "url": "https://a.test/" });
    let frame = json!({ "targetId": "FRAME", "type": "iframe", "url": "https://ads.test/" });
    let page = json!({ "targetId": "PAGE", "type": "page", "url": "https://a.test/" });
    assert!(hides_info(&OnePage, &popup));
    assert!(!hides_info(&OnePage, &frame));
    assert_eq!(
        visible_infos(&OnePage, &[popup, frame.clone(), page.clone()]),
        [frame, page]
    );
}

/// F4 (RB-050-05): a marker that carries a number (`about:blank#rb-render-7`) is hidden by its
/// prefix from the moment the target is created, before its id is registered.
#[test]
fn a_marker_prefix_hides_every_url_that_starts_with_it() {
    let filter = HiddenTargets::with_prefixes(&["rbrowser:"], &[], &["about:blank#rb-render-"]);
    assert!(filter.is_hidden("R1", "about:blank#rb-render-1"));
    assert!(filter.is_hidden("R2", "about:blank#rb-render-42"));
    assert!(!filter.is_hidden("V1", "about:blank#rb-view-1"));
    assert!(!filter.is_hidden("V2", "about:blank"));
    assert!(!filter.is_hidden("V3", "https://a.test/#rb-render-1"));
}
