use serde_json::json;

use super::*;
use crate::browser::{Browser, view};
use crate::columns;

const PAGE: &str = "http://127.0.0.1:18767/storage/index.html";

fn browser() -> Browser {
    let mut browser = Browser::default();
    browser.add_view(view(1, "a", "chromium", PAGE), "default");
    browser.add_view(view(2, "b", "chromium", PAGE), "default");
    browser.add_view(view(3, "c", "chromium", "https://example.com/"), "default");
    browser.add_view(view(4, "d", "webkit", PAGE), "webkit");
    browser
}

fn run_args(browser: &mut Browser, target: Target, args: &Value) -> Vec<Value> {
    let columns = columns::columns(browser);
    run(browser, &columns, target, &parse(args).expect("valid"))
}

#[test]
fn a_write_checks_its_fields() {
    for (args, field) in [
        (json!({ "name": "a", "value": "b" }), "kind"),
        (
            json!({ "kind": "indexedDB", "name": "a", "value": "b" }),
            "kind",
        ),
        (
            json!({ "kind": "localStorage", "name": "", "value": "b" }),
            "name",
        ),
        (json!({ "kind": "localStorage", "name": "a" }), "value"),
        (
            json!({ "kind": "localStorage", "name": "a", "value": 3 }),
            "value",
        ),
        (
            json!({ "kind": "localStorage", "name": "a", "delete": "yes" }),
            "delete",
        ),
        (
            json!({ "kind": "localStorage", "name": "a", "value": "b", "origin": "x" }),
            "origin",
        ),
        (
            json!({ "kind": "cookie", "name": "a;b", "value": "b" }),
            "name",
        ),
        (
            json!({ "kind": "cookie", "name": "a", "value": "b", "cookie": { "sameSite": "x" } }),
            "sameSite",
        ),
    ] {
        assert_eq!(parse(&args).err().as_deref(), Some(field), "{args}");
    }
    let removal =
        parse(&json!({ "kind": "sessionStorage", "name": "a", "delete": true })).expect("valid");
    assert_eq!(removal.value, None);
}

#[test]
fn a_storage_value_set_in_one_view_reads_back_there() {
    let mut browser = browser();
    let results = run_args(
        &mut browser,
        Target::View(1),
        &json!({ "kind": "localStorage", "name": "featureFlag", "value": "on" }),
    );
    assert_eq!(results, [json!({ "view": 1, "ok": true })]);
    assert_eq!(browser.local_item(1, "featureFlag").as_deref(), Some("on"));
}

#[test]
fn applied_to_all_views_each_answers_for_itself() {
    let mut browser = browser();
    let results = run_args(
        &mut browser,
        Target::All,
        &json!({ "kind": "sessionStorage", "name": "step", "value": "2",
                 "origin": "http://127.0.0.1:18767" }),
    );
    let outcome: Vec<(u64, bool, &str)> = results
        .iter()
        .map(|r| {
            (
                r["view"].as_u64().unwrap_or_default(),
                r["ok"] == true,
                r["error"].as_str().unwrap_or(""),
            )
        })
        .collect();
    assert_eq!(
        outcome,
        [
            (1, true, ""),
            (2, true, ""),
            (3, false, "plugin.storage-panel.errors.other_origin"),
            (4, false, "plugin.storage-panel.errors.unsupported_engine"),
        ]
    );
    assert_eq!(browser.session_item(2, "step").as_deref(), Some("2"));
}

#[test]
fn an_httponly_cookie_is_set_and_deleted_over_cdp() {
    let mut browser = browser();
    let set = run_args(
        &mut browser,
        Target::View(1),
        &json!({ "kind": "cookie", "name": "sid", "value": "abc",
                 "cookie": { "httpOnly": true, "path": "/" } }),
    );
    assert_eq!(set[0]["ok"], true);
    let jar = browser.cookies_of(2);
    assert_eq!(jar.len(), 1, "view 2 shares the session's jar");
    assert_eq!(jar[0]["httpOnly"], true);

    let deleted = run_args(
        &mut browser,
        Target::View(2),
        &json!({ "kind": "cookie", "name": "sid", "delete": true,
                 "cookie": { "domain": "127.0.0.1", "path": "/" } }),
    );
    assert_eq!(deleted[0]["ok"], true);
    assert!(browser.cookies_of(1).is_empty());
    let (_, method, params) = browser.cdp_sent.last().expect("sent");
    assert_eq!(method, "Network.deleteCookies");
    assert_eq!(params["domain"], "127.0.0.1");
}

#[test]
fn a_failing_view_names_the_hosts_code() {
    let mut browser = browser();
    browser.broken.insert(1);
    let results = run_args(
        &mut browser,
        Target::View(1),
        &json!({ "kind": "cookie", "name": "a", "value": "b" }),
    );
    assert_eq!(results[0]["error"], "plugins.error.host_failed");
}

#[test]
fn a_cookie_of_another_site_is_refused_before_any_cdp_call() {
    let mut browser = browser();
    for domain in [".other.example", "example.com", "127.0.0.2"] {
        let results = run_args(
            &mut browser,
            Target::View(1),
            &json!({ "kind": "cookie", "name": "a", "value": "b", "cookie": { "domain": domain } }),
        );
        assert_eq!(
            results[0]["error"], "plugin.storage-panel.errors.other_origin",
            "{domain}"
        );
    }
    let deleted = run_args(
        &mut browser,
        Target::View(3),
        &json!({ "kind": "cookie", "name": "a", "delete": true,
                 "cookie": { "domain": "127.0.0.1" } }),
    );
    assert_eq!(
        deleted[0]["error"],
        "plugin.storage-panel.errors.other_origin"
    );
    assert!(browser.cdp_sent.is_empty());
    let parent = run_args(
        &mut browser,
        Target::View(3),
        &json!({ "kind": "cookie", "name": "a", "value": "b",
                 "cookie": { "domain": ".example.com" } }),
    );
    assert_eq!(
        parent[0]["ok"], true,
        "a parent domain of the view's host is its own"
    );
}
