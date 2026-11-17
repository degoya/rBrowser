use serde_json::json;

use super::*;
use crate::fake::FakeHost;

#[test]
fn a_page_that_is_no_web_page_gives_nothing() {
    let mut host = FakeHost::default();
    let state: PageState = serde_json::from_value(json!({ "url": "about:blank" })).expect("state");
    let checked = check(&mut host, &Settings::default(), 1, &state, &[]);
    assert!(checked.findings.is_empty());
    assert!(host.fetched().is_empty() && host.inner.cdp_sent.is_empty());
}

#[test]
fn a_bare_page_gives_one_finding_per_missing_piece() {
    let mut host = FakeHost::default();
    let state: PageState = serde_json::from_value(
        json!({ "url": "https://a.test/", "secure": true, "sw": { "supported": true } }),
    )
    .expect("state");
    let checked = check(&mut host, &Settings::default(), 2, &state, &[]);
    let rules: Vec<&str> = checked.findings.iter().map(|f| f.rule.as_str()).collect();
    assert_eq!(
        rules,
        [
            "manifest.missing",
            "sw.missing",
            "meta.theme_color",
            "meta.apple_touch_icon"
        ]
    );
    assert!(checked.findings.iter().all(|f| f.view == Some(2)));
    assert!(checked.report.installability.checked);
    assert_eq!(checked.report.views, vec![2]);
}

#[test]
fn the_report_serialises_in_camel_case() {
    let report = PageReport::default();
    let value = serde_json::to_value(&report).expect("json");
    for key in [
        "serviceWorker",
        "installability",
        "themeColor",
        "appleTouchIcon",
    ] {
        assert!(value.get(key).is_some(), "{key}");
    }
}
