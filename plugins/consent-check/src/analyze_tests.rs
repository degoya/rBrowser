use crate::capture::from_probe;
use crate::rules;
use crate::settings::Settings;
use crate::site::host_rules;
use crate::testing::{self, SECRET};

use super::*;

fn analyzed(rejected: bool, settings: &Settings) -> Report {
    let run = from_probe(&testing::probe_args(rejected)).expect("a probe capture");
    analyze(&run, settings, &[])
}

fn rule_ids(report: &Report) -> Vec<&str> {
    report
        .findings
        .iter()
        .map(|finding| finding.rule.as_str())
        .collect()
}

fn param<'a>(finding: &'a rb_plugin_api::Finding, name: &str) -> &'a str {
    finding
        .params
        .iter()
        .find(|(key, _)| key == name)
        .map_or("", |(_, value)| value.as_str())
}

/// Acceptance: a script of a second site and a cookie kept a year, before consent - exactly
/// these two findings, and the cookie's value in none of them.
#[test]
fn the_acceptance_page_has_exactly_the_script_and_the_cookie() {
    let report = analyzed(false, &Settings::default());
    assert_eq!(
        rule_ids(&report),
        vec![rules::REQUEST_BEFORE, rules::COOKIE_BEFORE]
    );
    let script = &report.findings[0];
    assert_eq!(param(script, "host"), "cdn.tracker.test");
    assert_eq!(param(script, "kind"), "script");
    assert_eq!(param(script, "count"), "1");
    let cookie = &report.findings[1];
    assert_eq!(param(cookie, "name"), "_track");
    assert_eq!(param(cookie, "domain"), ".shop.example.test");
    assert_eq!(param(cookie, "days"), "365");
    let text = serde_json::to_string(&report.findings).expect("JSON");
    assert!(
        !text.contains(SECRET),
        "no cookie value in a finding: {text}"
    );
    assert!(
        text.contains(NOTICE_TEXT),
        "every finding carries the notice"
    );
}

/// Acceptance: a host of `consent-check.allowHosts` makes no finding.
#[test]
fn an_allowed_host_makes_no_finding() {
    let settings = Settings {
        allow_hosts: host_rules(&["cdn.tracker.test".to_owned()]),
        ..Settings::default()
    };
    let report = analyzed(false, &settings);
    assert_eq!(rule_ids(&report), vec![rules::COOKIE_BEFORE]);
    let row = report
        .before
        .hosts
        .iter()
        .find(|row| row.host == "cdn.tracker.test")
        .expect("the host is still listed");
    assert!(row.allowed);
}

#[test]
fn after_reject_new_third_party_requests_and_new_cookies_are_findings() {
    let report = analyzed(true, &Settings::default());
    assert_eq!(
        rule_ids(&report),
        vec![
            rules::REQUEST_BEFORE,
            rules::COOKIE_BEFORE,
            rules::REQUEST_AFTER,
            rules::COOKIE_AFTER,
        ]
    );
    let pixel = &report.findings[2];
    assert_eq!(param(pixel, "host"), "pixel.ads.test");
    assert_eq!(param(pixel, "kind"), "pixel");
    let late = &report.findings[3];
    assert_eq!(param(late, "name"), "_ads");
    assert_eq!(late.message, rules::message(rules::COOKIE_AFTER, true));
    let data = late.data.as_deref().unwrap_or_default();
    assert!(!data.contains("uid=42") && !data.contains(SECRET));
}

#[test]
fn the_differences_between_the_phases_are_named() {
    let report = analyzed(true, &Settings::default());
    let diff = report.diff.expect("a diff after reject");
    assert_eq!(diff.hosts_added, vec!["pixel.ads.test pixel".to_owned()]);
    assert_eq!(
        diff.hosts_removed,
        vec!["cdn.tracker.test script".to_owned()]
    );
    assert_eq!(
        diff.cookies_added,
        vec![
            "OptanonConsent .shop.example.test".to_owned(),
            "_ads .ads.test".to_owned()
        ]
    );
    assert!(diff.cookies_removed.is_empty());
    assert_eq!(diff.storage_added, vec!["consent-state".to_owned()]);
}

#[test]
fn essential_cookies_are_listed_but_make_no_finding() {
    let settings = Settings {
        essential_cookies: vec!["_track".to_owned()],
        ..Settings::default()
    };
    let report = analyzed(false, &settings);
    assert_eq!(rule_ids(&report), vec![rules::REQUEST_BEFORE]);
    assert!(report.before.cookies[0].essential);
}

#[test]
fn rules_switched_off_make_no_finding() {
    let run = from_probe(&testing::probe_args(true)).expect("a probe capture");
    let enabled = vec![rules::COOKIE_BEFORE.to_owned()];
    let report = analyze(&run, &Settings::default(), &enabled);
    assert_eq!(rule_ids(&report), vec![rules::COOKIE_BEFORE]);
}

#[test]
fn requests_of_one_host_and_kind_are_one_row() {
    let mut run = from_probe(&testing::probe_args(false)).expect("a probe capture");
    run.before.requests.push(crate::capture::Request {
        url: "https://cdn.tracker.test/t2.js?x=1".to_owned(),
        resource_type: "Script".to_owned(),
    });
    let report = analyze(&run, &Settings::default(), &[]);
    let rows: Vec<_> = report
        .before
        .hosts
        .iter()
        .filter(|row| row.host == "cdn.tracker.test")
        .collect();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].count, 2);
    assert_eq!(rows[0].example, "https://cdn.tracker.test/t.js");
    assert_eq!(report.before.first_party, 2);
}

#[test]
fn a_session_cookie_has_its_own_message() {
    let mut run = from_probe(&testing::probe_args(false)).expect("a probe capture");
    run.before.cookies[0].expires = None;
    let report = analyze(&run, &Settings::default(), &[]);
    let cookie = &report.findings[1];
    assert_eq!(cookie.message, rules::message(rules::COOKIE_BEFORE, true));
    assert!(cookie.params.iter().all(|(key, _)| key != "days"));
}
