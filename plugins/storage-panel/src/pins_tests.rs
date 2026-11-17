use serde_json::json;

use super::*;
use crate::browser::{Browser, RECORD, view};
use crate::columns;

const ORIGIN: &str = "http://127.0.0.1:18767";
const PAGE: &str = "http://127.0.0.1:18767/storage/index.html";

fn flag() -> Pin {
    Pin::parse(
        &json!({ "kind": "localStorage", "origin": ORIGIN, "key": "featureFlag",
                        "value": "on" }),
    )
    .expect("valid")
}

fn consent(session: Option<&str>) -> Pin {
    Pin::parse(
        &json!({ "kind": "cookie", "origin": ORIGIN, "key": "consent", "value": "yes",
                        "session": session, "cookie": { "path": "/" } }),
    )
    .expect("valid")
}

fn browser() -> Browser {
    let mut browser = Browser::default();
    browser.project.insert(
        columns::CONFIG_PATH.into(),
        br#"{"breakpoints":[{"id":"a"},{"id":"b","session":"admin"}]}"#.to_vec(),
    );
    browser.add_view(view(1, "a", "chromium", PAGE), "default");
    browser.add_view(view(2, "b", "chromium", PAGE), "admin");
    browser
}

#[test]
fn a_pin_reads_back_as_it_was_written() {
    let pin = consent(Some("admin"));
    assert_eq!(Pin::parse(&pin.to_json()), Ok(pin.clone()));
    assert_eq!(pin.cookie["path"], "/");
    assert!(flag().enabled, "a pin is on unless it says otherwise");
    assert_ne!(
        pin.id(),
        consent(None).id(),
        "the session is part of the id"
    );
}

#[test]
fn a_pin_that_does_not_hold_is_refused_by_field() {
    let base = json!({ "kind": "localStorage", "origin": ORIGIN, "key": "k", "value": "v" });
    for (field, bad, reason) in [
        ("kind", json!("indexedDB"), "kind"),
        ("origin", json!("http://127.0.0.1:18767/x"), "origin"),
        ("key", json!(7), "key"),
        ("value", json!(null), "value"),
        ("session", json!("a b"), "session"),
        ("enabled", json!("yes"), "enabled"),
        ("key", json!(""), "value"),
        ("value", json!("v".repeat(MAX_PIN_CHARS + 1)), "value"),
    ] {
        let mut entry = base.clone();
        entry[field] = bad;
        assert_eq!(Pin::parse(&entry).err().as_deref(), Some(reason), "{field}");
    }
    let bad_cookie = json!({ "kind": "cookie", "origin": ORIGIN, "key": "a;b", "value": "v" });
    assert_eq!(Pin::parse(&bad_cookie).err().as_deref(), Some("name"));
}

#[test]
fn pins_apply_by_origin_session_and_switch() {
    let flag = flag();
    assert!(flag.applies("default", ORIGIN));
    assert!(!flag.applies("default", "https://example.com"));
    let admin_only = consent(Some("admin"));
    assert!(admin_only.applies("admin", ORIGIN));
    assert!(!admin_only.applies("default", ORIGIN));
    let mut off = consent(None);
    off.enabled = false;
    assert!(!off.applies("default", ORIGIN));
}

#[test]
fn the_record_holds_the_storage_pins_of_the_origin_only() {
    let mut tab = flag();
    tab.kind = Kind::Session;
    tab.key = "step".into();
    let pins = [flag(), tab, consent(None)];
    let record = record(&pins, "default", ORIGIN);
    assert_eq!(
        record,
        json!({ "local": { "featureFlag": "on" }, "session": { "step": "on" } })
    );
    assert!(record_is_empty(&super::record(
        &pins,
        "default",
        "https://example.com"
    )));
}

#[test]
fn pins_are_saved_and_loaded_and_bad_entries_dropped_without_values() {
    let mut browser = browser();
    save(&mut browser, &[flag(), consent(None)]).expect("saved");
    assert_eq!(load(&mut browser), [flag(), consent(None)]);

    browser.project.insert(
        PINS_PATH.into(),
        json!({ "version": 1, "pins": [flag().to_json(), flag().to_json(),
            { "kind": "localStorage", "origin": "nope", "key": "token", "value": "TOPSECRET" }] })
        .to_string()
        .into_bytes(),
    );
    assert_eq!(load(&mut browser), [flag()]);
    assert_eq!(browser.logs.len(), 1);
    assert!(!browser.logs[0].1.contains("TOPSECRET"));
}

#[test]
fn applying_sets_cookies_and_storage_and_keeps_the_record_for_the_next_document() {
    let mut browser = browser();
    let columns = columns::columns(&mut browser);
    let pins = [flag(), consent(Some("admin"))];
    for column in &columns {
        assert!(apply_view(&mut browser, column, &pins).is_empty());
    }
    assert_eq!(browser.local_item(1, "featureFlag").as_deref(), Some("on"));
    assert_eq!(browser.local_item(2, "featureFlag").as_deref(), Some("on"));
    assert!(
        browser.cookies_of(1).is_empty(),
        "the cookie is admin's only"
    );
    assert_eq!(browser.cookies_of(2)[0]["value"], "yes");
    assert!(browser.session_item(1, RECORD).is_some());

    // Unpinned: the next document finds no record.
    for column in &columns {
        assert!(apply_view(&mut browser, column, &[]).is_empty());
    }
    assert!(browser.session_item(1, RECORD).is_none());
}

#[test]
fn a_pin_holds_its_row() {
    let mut browser = browser();
    browser.set_local(1, "featureFlag", "off");
    browser.set_cookie(
        1,
        json!({ "name": "consent", "value": "no", "domain": "127.0.0.1", "path": "/" }),
    );
    let columns = columns::columns(&mut browser);
    let data = read::read_all(
        &mut browser,
        &columns,
        columns::Target::All,
        &Kind::ALL,
        true,
    );
    let local = crate::matrix::rows(Kind::Local, &data);
    let cookies = crate::matrix::rows(Kind::Cookies, &data);
    assert!(flag().holds(&local[0]));
    assert!(consent(None).holds(&cookies[0]));
    assert!(!flag().holds(&cookies[0]));
}

#[test]
fn pins_of_an_untrusted_workspace_are_neither_loaded_nor_applied() {
    // A cloned repository can commit `.rbrowser/out/storage-panel/pins.json` (review M3,
    // Charge 7): without workspace trust it is not read.
    let mut browser = browser();
    save(&mut browser, &[flag(), consent(None)]).expect("saved");
    browser.untrusted = true;
    assert!(load(&mut browser).is_empty());
    let refused = load_trusted(&mut browser).expect_err("refused");
    assert_eq!(
        refused.code.as_deref(),
        Some(rb_plugin_api::codes::UNTRUSTED_WORKSPACE)
    );
    browser.untrusted = false;
    assert_eq!(load(&mut browser), [flag(), consent(None)], "trusted again");
}
