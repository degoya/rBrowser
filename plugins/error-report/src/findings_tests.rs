use rb_plugin_api::{Severity, ViewInfo};
use serde_json::{Value, json};

use super::*;
use crate::config::Settings;
use crate::feed::{self, FEED_KEY};
use crate::report::build;

fn views() -> Vec<ViewInfo> {
    ["mobile", "desktop"]
        .iter()
        .zip(1..)
        .map(|(name, id)| ViewInfo {
            id,
            label: (*name).to_owned(),
            breakpoint: (*name).to_owned(),
            engine: "chromium".to_owned(),
            url: "https://a.test/".to_owned(),
            ..ViewInfo::default()
        })
        .collect()
}

fn report(views_json: &[Value]) -> Report {
    let feed = feed::read(&json!({ FEED_KEY: { "views": views_json } })).expect("a capture");
    build(&feed, &views(), &Settings::default(), 0.0)
}

#[test]
fn one_rule_per_category_with_its_title_key() {
    let rules = rules();
    assert_eq!(rules.len(), Category::ALL.len());
    assert_eq!(rules[0].id, "js-exception");
    assert_eq!(rules[0].title, "plugin.error-report.rules.js_exception");
    assert!(
        rules
            .iter()
            .all(|rule| rb_plugin_api::check::valid_rule_id(&rule.id))
    );
}

#[test]
fn a_finding_per_view_with_one_path_and_only_in_when_partial() {
    let report = report(&[
        json!({ "view": 1,
            "errors": [{ "text": "Uncaught Error: x", "url": "https://a.test/a.js", "time": 1.0 }],
            "network": [{ "url": "https://a.test/m.png", "status": 404, "time": 1.0 }] }),
        json!({ "view": 2,
            "errors": [{ "text": "Uncaught Error: x", "url": "https://a.test/a.js", "time": 2.0 }] }),
    ]);
    let found = findings(&report, None, &[]);
    assert_eq!(found.len(), 3);
    assert_eq!(found[0].rule, "js-exception");
    assert_eq!(found[0].path, found[1].path);
    assert_eq!((found[0].view, found[1].view), (Some(1), Some(2)));
    assert_eq!(
        found[0].message,
        "plugin.error-report.findings.js_exception"
    );
    assert_eq!(found[0].url.as_deref(), Some("https://a.test/"));
    let only = &found[2];
    assert_eq!(only.rule, "http-not-found");
    assert_eq!(only.severity, Severity::Error);
    assert_eq!(
        only.message,
        "plugin.error-report.findings.http_not_found_only"
    );
    assert!(
        only.params
            .contains(&("views".to_owned(), "mobile".to_owned()))
    );
    let data: Value = serde_json::from_str(only.data.as_deref().unwrap_or("null")).expect("JSON");
    assert_eq!(data["request"]["status"], 404);
}

#[test]
fn a_named_view_and_the_wanted_rules_narrow_the_findings() {
    let report = report(&[
        json!({ "view": 1, "errors": [{ "text": "Uncaught Error: x", "time": 1.0 }],
                "network": [{ "url": "https://a.test/m.png", "status": 404, "time": 1.0 }] }),
        json!({ "view": 2, "errors": [{ "text": "Uncaught Error: x", "time": 1.0 }] }),
    ]);
    assert_eq!(findings(&report, Some(2), &[]).len(), 1);
    let wanted = vec!["http-not-found".to_owned()];
    let found = findings(&report, None, &wanted);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].rule, "http-not-found");
}
