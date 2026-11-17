use serde_json::json;

use super::*;
use crate::browser::{Browser, view};
use crate::pins::{PINS_PATH, Pin};

const ORIGIN: &str = "http://127.0.0.1:18767";
const PAGE: &str = "http://127.0.0.1:18767/storage/index.html";

fn browser(pins: &[Value]) -> Browser {
    let mut browser = Browser::default();
    browser.add_view(view(1, "a", "chromium", PAGE), "default");
    browser.project.insert(
        PINS_PATH.into(),
        json!({ "version": 1, "pins": pins })
            .to_string()
            .into_bytes(),
    );
    browser
}

fn loaded(origin: &str) -> Value {
    json!({ "view": 1, "message": { "type": "loaded", "origin": origin } })
}

#[test]
fn a_starting_document_gets_its_origins_storage_pins_and_its_cookies_set() {
    let flag = json!({ "kind": "localStorage", "origin": ORIGIN, "key": "featureFlag",
                       "value": "on" });
    let consent = json!({ "kind": "cookie", "origin": ORIGIN, "key": "consent", "value": "yes" });
    let elsewhere = json!({ "kind": "localStorage", "origin": "https://example.com",
                            "key": "x", "value": "y" });
    let mut browser = browser(&[flag, consent, elsewhere]);
    let answer = request(&mut browser, &loaded(ORIGIN)).expect("answers");
    assert_eq!(
        answer["record"],
        json!({ "origin": ORIGIN, "local": { "featureFlag": "on" }, "session": {} })
    );
    assert_eq!(answer["$post"][0]["target"], "panel");
    assert_eq!(answer["$post"][0]["message"]["view"], 1);
    assert_eq!(browser.cookies_of(1)[0]["name"], "consent");
}

#[test]
fn nothing_is_pinned_without_pins_an_origin_or_the_switch() {
    let flag = Pin::parse(
        &json!({ "kind": "localStorage", "origin": ORIGIN, "key": "k",
                                   "value": "v" }),
    )
    .expect("valid")
    .to_json();
    let mut empty = browser(&[]);
    assert_eq!(
        request(&mut empty, &loaded(ORIGIN)).expect("answers")["record"],
        Value::Null
    );

    let mut browser = browser(&[flag]);
    assert_eq!(
        request(&mut browser, &loaded("null")).expect("answers")["record"],
        Value::Null
    );
    browser
        .settings
        .insert("pinsEnabled".into(), "false".into());
    assert_eq!(
        request(&mut browser, &loaded(ORIGIN)).expect("answers")["record"],
        Value::Null
    );
    assert!(browser.cdp_sent.is_empty());
}

#[test]
fn a_message_that_is_no_load_is_refused() {
    let mut browser = browser(&[]);
    for args in [
        json!({ "view": 1, "message": { "type": "hello" } }),
        json!({ "message": { "type": "loaded", "origin": ORIGIN } }),
    ] {
        let failure = request(&mut browser, &args).expect_err("refused");
        assert_eq!(
            failure.code.as_deref(),
            Some("plugin.storage-panel.errors.bad_args")
        );
    }
}

#[test]
fn an_untrusted_workspace_sets_no_pinned_cookie_or_value() {
    let flag = json!({ "kind": "localStorage", "origin": ORIGIN, "key": "featureFlag",
                       "value": "on" });
    let consent = json!({ "kind": "cookie", "origin": ORIGIN, "key": "consent", "value": "yes",
                          "cookie": { "domain": ".0.0.1" } });
    let mut browser = browser(&[flag, consent]);
    browser.untrusted = true;
    let answer = request(&mut browser, &loaded(ORIGIN)).expect("answers");
    assert_eq!(answer["record"], Value::Null, "review M3 of Charge 7");
    assert!(browser.cdp_sent.is_empty(), "no cookie set");
    assert!(browser.cookies_of(1).is_empty());
}
