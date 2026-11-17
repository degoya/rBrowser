use std::collections::BTreeMap;

use rb_plugin_api::check::valid_rule_id;

use super::*;
use crate::feed::Entry;
use crate::group::group;

fn entry(view: ViewId, level: Level, kind: Kind, text: &str) -> Entry {
    Entry {
        view,
        level,
        kind,
        text: text.to_owned(),
        url: Some("http://x.test/console.html".to_owned()),
        line: Some(20),
        column: Some(5),
        stack: None,
        time: f64::from(view),
    }
}

fn groups() -> Vec<Group> {
    group(&[
        entry(
            1,
            Level::Error,
            Kind::Exception,
            "Uncaught Error: narrow 320",
        ),
        entry(
            2,
            Level::Error,
            Kind::Exception,
            "Uncaught Error: narrow 375",
        ),
        entry(1, Level::Error, Kind::Console, "save failed"),
        entry(3, Level::Warning, Kind::Console, "Deprecated"),
        entry(3, Level::Error, Kind::Browser, "Failed to load resource"),
    ])
}

fn urls() -> BTreeMap<ViewId, String> {
    (1..=3)
        .map(|view| (view, "http://x.test/console.html".to_owned()))
        .collect()
}

#[test]
fn the_rules_are_valid_and_named_in_the_locales() {
    let locales = include_str!("../locales/en.json");
    for rule in rules() {
        assert!(valid_rule_id(&rule.id), "{}", rule.id);
        let leaf = rule.title.rsplit('.').next().unwrap_or_default();
        assert!(locales.contains(&format!("\"{leaf}\"")), "{}", rule.title);
    }
}

#[test]
fn an_uncaught_exception_is_a_js_error_in_each_view_that_threw() {
    let found = findings(&groups(), None, &[], &urls());
    let js: Vec<&Finding> = found.iter().filter(|f| f.rule == JS_ERROR).collect();
    assert_eq!(
        js.iter().map(|f| f.view).collect::<Vec<_>>(),
        [Some(1), Some(2)]
    );
    assert!(js.iter().all(|f| f.severity == Severity::Error));
    // One path for the group: the check runner merges the views into one finding.
    assert_eq!(js[0].path, js[1].path);
    assert_eq!(js[0].message, "plugin.console.findings.js_error");
    assert_eq!(js[0].url.as_deref(), Some("http://x.test/console.html"));
    let params: BTreeMap<_, _> = js[0].params.iter().cloned().collect();
    assert_eq!(params["text"], "Uncaught Error: narrow 320");
    assert_eq!(params["source"], "http://x.test/console.html:20");
}

#[test]
fn console_error_is_a_warning_and_browser_errors_are_none() {
    let found = findings(&groups(), None, &[], &urls());
    let console: Vec<&Finding> = found.iter().filter(|f| f.rule == CONSOLE_ERROR).collect();
    assert_eq!(console.len(), 1);
    assert_eq!(console[0].severity, Severity::Warning);
    assert_eq!(found.len(), 3);
}

#[test]
fn a_run_for_one_view_reports_that_view_only() {
    let found = findings(&groups(), Some(2), &[], &urls());
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].view, Some(2));
}

#[test]
fn rules_switched_off_report_nothing() {
    let found = findings(&groups(), None, &[CONSOLE_ERROR.to_owned()], &urls());
    assert!(found.iter().all(|f| f.rule == CONSOLE_ERROR));
    assert_eq!(found.len(), 1);
}

#[test]
fn an_error_rbrowser_logged_is_no_finding_of_the_page() {
    // RB-080-66: a plugin's failure is rBrowser's, not the page's.
    let groups = group(&[entry(
        1,
        Level::Error,
        Kind::Rbrowser,
        "The plugin took too long",
    )]);
    assert!(findings(&groups, None, &[], &urls()).is_empty());
}
