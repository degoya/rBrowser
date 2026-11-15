use serde_json::json;

use super::*;
use crate::cookies::model::cookies_of;

const NOW: u64 = 1_800_000_000;

fn chrome(fields: serde_json::Value) -> ChromeCookie {
    let mut cookie = json!({
        "name": "sid", "value": "hunter2", "domain": "127.0.0.1", "path": "/",
        "expires": -1, "httpOnly": true, "secure": false, "session": true,
    });
    if let (Some(base), Some(extra)) = (cookie.as_object_mut(), fields.as_object()) {
        base.extend(extra.clone());
    }
    cookies_of(&json!({ "cookies": [cookie] }))
        .pop()
        .expect("a cookie")
}

/// The mapping table of the module docs, row by row.
#[test]
fn the_mapping_table_holds_for_firefox_and_webkit() {
    // A session login cookie of a local dev server.
    let login = chrome(json!({}));
    let firefox = to_bidi(&login, NOW).expect("copied");
    assert_eq!(
        (
            firefox.domain.as_str(),
            firefox.path.as_str(),
            firefox.http_only,
            firefox.secure
        ),
        ("127.0.0.1", "/", true, false)
    );
    assert_eq!(firefox.expiry, None, "a session cookie stays one");
    assert_eq!(
        firefox.same_site, "lax",
        "absent is Chromium's lax by default"
    );
    let webkit = to_playwright(&login, NOW).expect("copied");
    assert_eq!(webkit.expires, -1);
    assert_eq!(webkit.same_site, "Lax");
    // A persistent domain cookie with its expiry in whole seconds.
    let remembered = chrome(json!({
        "domain": ".example.com", "expires": 1_900_000_000.75, "session": false,
        "sameSite": "Strict", "secure": true, "httpOnly": false,
    }));
    let firefox = to_bidi(&remembered, NOW).expect("copied");
    assert_eq!(firefox.domain, ".example.com");
    assert_eq!(firefox.expiry, Some(1_900_000_000));
    assert_eq!(firefox.same_site, "strict");
    assert!(firefox.secure && !firefox.http_only);
    let webkit = to_playwright(&remembered, NOW).expect("copied");
    assert_eq!(webkit.expires, 1_900_000_000);
    assert_eq!(webkit.same_site, "Strict");
    assert!(webkit.secure, "Secure stays off loopback");
}

#[test]
fn same_site_none_and_secure_on_localhost_is_kept_where_the_engine_honours_it() {
    let cookie = chrome(json!({ "domain": "localhost", "sameSite": "None", "secure": true }));
    let firefox = to_bidi(&cookie, NOW).expect("copied");
    assert_eq!((firefox.same_site.as_str(), firefox.secure), ("none", true));
    let webkit = to_playwright(&cookie, NOW).expect("copied");
    assert_eq!(
        (webkit.same_site, webkit.secure),
        ("None", false),
        "WebKit sends no Secure cookie over plain HTTP, Chromium does on loopback"
    );
    for host in ["localhost", ".app.localhost", "127.0.0.1", "[::1]"] {
        assert!(is_loopback(host), "{host}");
    }
    for host in ["example.com", "localhost.example.com", "127.0.0.10"] {
        assert!(!is_loopback(host), "{host}");
    }
}

#[test]
fn partitioned_expired_and_malformed_cookies_are_skipped_with_their_reason() {
    let chips = chrome(json!({ "partitionKey": { "topLevelSite": "https://a.example" } }));
    assert_eq!(to_bidi(&chips, NOW), Err(SkipReason::Partitioned));
    let opaque = chrome(json!({ "partitionKeyOpaque": true }));
    assert_eq!(
        to_playwright(&opaque, NOW).map(|_| ()),
        Err(SkipReason::Partitioned)
    );
    let old = chrome(json!({ "expires": 1_000.0, "session": false }));
    assert_eq!(to_bidi(&old, NOW), Err(SkipReason::Expired));
    for bad in [
        json!({ "name": "" }),
        json!({ "domain": "." }),
        json!({ "path": "account" }),
    ] {
        assert_eq!(check(&chrome(bad), NOW), Err(SkipReason::Invalid));
    }
}

#[test]
fn the_webkit_cookie_is_what_the_sidecar_reads_and_hides_its_value() {
    let webkit = to_playwright(&chrome(json!({})), NOW).expect("copied");
    assert_eq!(
        serde_json::to_value(&webkit).expect("serializes"),
        json!({ "name": "sid", "value": "hunter2", "domain": "127.0.0.1", "path": "/",
                "expires": -1, "httpOnly": true, "secure": false, "sameSite": "Lax" })
    );
    assert!(!format!("{webkit:?}").contains("hunter2"));
}
