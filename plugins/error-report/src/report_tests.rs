use rb_plugin_api::{Severity, ViewInfo};
use serde_json::{Value, json};

use super::*;
use crate::config::{RuleSetting, Settings};
use crate::feed::{self, FEED_KEY};

const PAGE: &str = "https://a.test/";

fn view(id: u32, breakpoint: &str, engine: &str) -> ViewInfo {
    ViewInfo {
        id,
        label: format!("View {id}"),
        breakpoint: breakpoint.to_owned(),
        engine: engine.to_owned(),
        width: 375 * id,
        height: 800,
        dpr: 1.0,
        url: PAGE.to_owned(),
        title: String::new(),
    }
}

fn four() -> Vec<ViewInfo> {
    vec![
        view(1, "mobile", "chromium"),
        view(2, "tablet", "chromium"),
        view(3, "laptop", "chromium"),
        view(4, "desktop", "chromium"),
    ]
}

fn capture(views: &[Value]) -> Feed {
    feed::read(&json!({ FEED_KEY: { "views": views } })).expect("a capture")
}

fn exception(id: u32, at: f64) -> Value {
    json!({ "view": id, "errors": [{ "text": format!("Uncaught Error: boom at {}", 300 * id),
             "url": "https://a.test/app.js", "line": 3, "time": at }] })
}

#[test]
fn the_same_error_in_four_views_is_one_entry_with_four_views() {
    let feed = capture(&(1..=4).map(|id| exception(id, 10.0)).collect::<Vec<_>>());
    let report = build(&feed, &four(), &Settings::default(), 0.0);
    assert_eq!(report.entries.len(), 1);
    let entry = &report.entries[0];
    assert_eq!(entry.views(), [1, 2, 3, 4]);
    assert_eq!(entry.count(), 4);
    assert_eq!(entry.only_in, None);
    assert_eq!(entry.severity, Severity::Error);
}

#[test]
fn a_404_only_in_the_mobile_view_says_so() {
    let mut views: Vec<Value> = (2..=4).map(|id| json!({ "view": id })).collect();
    views.push(
        json!({ "view": 1, "network": [{ "url": "https://a.test/img/hero-375.png",
        "resourceType": "Image", "status": 404, "time": 10.0 }] }),
    );
    let report = build(&capture(&views), &four(), &Settings::default(), 0.0);
    assert_eq!(report.entries.len(), 1);
    assert_eq!(report.entries[0].only_in, Some(vec!["mobile".to_owned()]));
    let answer = to_json(&report, None, 100);
    assert_eq!(answer["entries"][0]["onlyIn"], json!(["mobile"]));
    assert_eq!(answer["entries"][0]["request"]["status"], 404);
    assert_eq!(answer["summary"]["http-not-found"], 1);
    assert_eq!(answer["severities"]["error"], 1);
    // The view filter of MCP's target.
    assert_eq!(to_json(&report, Some(2), 100)["total"], 0);
    assert_eq!(to_json(&report, Some(1), 100)["total"], 1);
}

#[test]
fn a_view_that_captures_nothing_is_no_reason_for_only_in() {
    let mut views = four();
    views[3].engine = "firefox".to_owned();
    let feed = capture(
        &(1..=4)
            .map(|id| {
                if id == 4 {
                    json!({ "view": 4 })
                } else {
                    exception(id, 10.0)
                }
            })
            .collect::<Vec<_>>(),
    );
    let report = build(&feed, &views, &Settings::default(), 0.0);
    assert_eq!(report.capturing, [1, 2, 3]);
    assert_eq!(report.entries[0].only_in, None);
}

#[test]
fn an_ignore_pattern_drops_the_matching_entry() {
    let feed = capture(&[json!({ "view": 1,
        "network": [{ "url": "https://ads.test/pixel.gif", "status": 404, "time": 1.0 },
                    { "url": "https://a.test/x.css", "status": 500, "time": 1.0 }],
        "errors": [{ "text": "Uncaught ResizeObserver loop limit exceeded", "time": 1.0 }] })]);
    let settings = Settings {
        ignore: vec!["*ads.test*".to_owned(), "*resizeobserver*".to_owned()],
        ..Settings::default()
    };
    let report = build(&feed, &four(), &settings, 0.0);
    let rules: Vec<&str> = report.entries.iter().map(|e| e.category.rule()).collect();
    assert_eq!(rules, ["http-server-error"]);
}

#[test]
fn only_what_came_after_the_page_loaded_and_after_the_clear_mark() {
    let feed = capture(&[json!({ "view": 1,
        "errors": [{ "text": "Uncaught Error: old page", "time": 5.0 },
                   { "text": "Uncaught Error: cleared", "time": 15.0 },
                   { "text": "Uncaught Error: new", "time": 25.0 }],
        "network": [{ "url": "https://a.test/#top", "resourceType": "Document", "status": 200,
                      "time": 10.0 }] })]);
    let texts = |report: &Report| -> Vec<String> {
        report.entries.iter().map(|e| e.text.clone()).collect()
    };
    let report = build(&feed, &four(), &Settings::default(), 20.0);
    assert_eq!(texts(&report), ["Uncaught Error: new"]);
    let report = build(&feed, &four(), &Settings::default(), 0.0);
    assert_eq!(texts(&report).len(), 2);
    let all = Settings {
        since_navigation: false,
        ..Settings::default()
    };
    assert_eq!(texts(&build(&feed, &four(), &all, 0.0)).len(), 3);
}

#[test]
fn other_sites_off_rules_and_severities_follow_the_settings() {
    let feed = capture(&[json!({ "view": 1,
        "network": [{ "url": "https://cdn.other.test/lib.js", "status": 404, "time": 1.0 },
                    { "url": "https://static.a.test/app.css", "status": 404, "time": 1.0 }],
        "issues": [{ "code": "DeprecationIssue", "kind": "XHRSynchronous", "time": 1.0 },
                   { "code": "ContentSecurityPolicyIssue", "kind": "img-src",
                     "url": "https://a.test/p.png", "time": 1.0 }] })]);
    let mut settings = Settings {
        third_party: false,
        ..Settings::default()
    };
    settings
        .rules
        .insert("deprecation".to_owned(), RuleSetting::Off);
    settings.rules.insert(
        "csp-violation".to_owned(),
        RuleSetting::Severity(Severity::Error),
    );
    let report = build(&feed, &four(), &settings, 0.0);
    let found: Vec<(&str, Severity)> = report
        .entries
        .iter()
        .map(|e| (e.category.rule(), e.severity))
        .collect();
    assert_eq!(
        found,
        [
            ("http-not-found", Severity::Error),
            ("csp-violation", Severity::Error)
        ]
    );
    assert_eq!(
        report.entries[0].url.as_deref(),
        Some("https://static.a.test/app.css")
    );
}

#[test]
fn the_answer_is_cut_to_its_limit_and_says_so() {
    let feed = capture(&[json!({ "view": 1, "errors": [
        { "text": "Uncaught Error: a", "time": 1.0 },
        { "text": "Uncaught Error: b", "time": 2.0 }] })]);
    let answer = to_json(&build(&feed, &four(), &Settings::default(), 0.0), None, 1);
    assert_eq!(answer["total"], 2);
    assert_eq!(answer["truncated"], true);
    assert_eq!(answer["entries"].as_array().map(Vec::len), Some(1));
    assert_eq!(answer["captured"], true);
    assert_eq!(answer["views"][0]["captures"], true);
}
