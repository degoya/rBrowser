use serde_json::json;

use super::*;
use crate::cookies::model::cookies_of;

fn chrome_cookies() -> Vec<ChromeCookie> {
    cookies_of(&json!({ "cookies": [
        { "name": "sid", "value": "cookie-secret-1", "domain": "staging.example.test", "path": "/",
          "expires": -1, "httpOnly": true, "secure": true, "session": true, "sameSite": "Lax" },
        { "name": "pref", "value": "dark", "domain": ".example.test", "path": "/",
          "expires": 1_900_000_000.5, "httpOnly": false, "secure": false, "session": false },
        { "name": "chips", "value": "x", "domain": "embed.example.test", "path": "/",
          "partitionKey": { "topLevelSite": "https://example.test" } }
    ]}))
}

#[test]
fn chromium_cookies_become_playwright_cookies_without_the_partitioned_ones() {
    let state = StorageState::from_chrome(&chrome_cookies(), Vec::new());
    assert_eq!(state.cookies.len(), 2);
    let sid = &state.cookies[0];
    assert_eq!(sid.name, "sid");
    assert_eq!(sid.value, "cookie-secret-1");
    assert!((sid.expires + 1.0).abs() < f64::EPSILON, "a session cookie");
    assert!(sid.http_only && sid.secure);
    assert_eq!(sid.same_site, "Lax");
    let pref = &state.cookies[1];
    assert_eq!(pref.domain, ".example.test");
    assert_eq!(
        pref.same_site, "Lax",
        "Chromium's default when the site set none"
    );
    assert!((pref.expires - 1_900_000_000.5).abs() < f64::EPSILON);
}

#[test]
fn the_file_is_playwrights_storage_state() {
    let state = StorageState::from_chrome(
        &chrome_cookies()[..1],
        vec![OriginState {
            origin: "https://staging.example.test".to_owned(),
            local_storage: vec![NameValue {
                name: "token".to_owned(),
                value: "ls-secret".to_owned(),
            }],
        }],
    );
    let written = serde_json::to_value(&state).expect("serializes");
    assert_eq!(
        written,
        json!({
            "cookies": [{ "name": "sid", "value": "cookie-secret-1",
                "domain": "staging.example.test", "path": "/", "expires": -1.0,
                "httpOnly": true, "secure": true, "sameSite": "Lax" }],
            "origins": [{ "origin": "https://staging.example.test",
                "localStorage": [{ "name": "token", "value": "ls-secret" }] }]
        })
    );
    // A file Playwright wrote, with fields this version does not know, reads.
    let read: StorageState = serde_json::from_value(json!({
        "cookies": [{ "name": "a", "value": "b", "domain": "x.test", "path": "/",
            "expires": -1, "httpOnly": false, "secure": false, "sameSite": "None",
            "partitionKey": "https://x.test" }],
        "origins": [{ "origin": "https://x.test", "localStorage": [], "indexedDB": [] }]
    }))
    .expect("reads");
    assert_eq!(read.check(), Ok(()));
}

#[test]
fn the_debug_output_counts_and_never_shows_a_value() {
    let state = StorageState::from_chrome(&chrome_cookies(), Vec::new());
    let shown = format!("{state:?}");
    assert!(!shown.contains("cookie-secret-1"), "{shown}");
    assert!(!shown.contains("sid"), "{shown}");
    assert!(shown.contains('2'), "{shown}");
}

#[test]
fn set_cookies_carries_every_field_cdp_takes() {
    let state = StorageState::from_chrome(&chrome_cookies(), Vec::new());
    let params = state.set_cookies_params();
    assert_eq!(
        params["cookies"][0],
        json!({ "name": "sid", "value": "cookie-secret-1", "domain": "staging.example.test",
            "path": "/", "secure": true, "httpOnly": true, "sameSite": "Lax" }),
        "a session cookie has no expiry"
    );
    assert_eq!(params["cookies"][1]["expires"], json!(1_900_000_000.5));
}

#[test]
fn a_hostile_file_is_refused() {
    let cookie = |name: &str, domain: &str, path: &str, same_site: &str| StateCookie {
        name: name.to_owned(),
        value: "v".to_owned(),
        domain: domain.to_owned(),
        path: path.to_owned(),
        expires: -1.0,
        http_only: false,
        secure: false,
        same_site: same_site.to_owned(),
    };
    let with =
        |cookies: Vec<StateCookie>, origins: Vec<OriginState>| StorageState { cookies, origins };
    assert!(
        with(vec![cookie("", "x.test", "/", "Lax")], vec![])
            .check()
            .is_err()
    );
    assert!(
        with(vec![cookie("a", "", "/", "Lax")], vec![])
            .check()
            .is_err()
    );
    assert!(
        with(vec![cookie("a", "x.test", "rel", "Lax")], vec![])
            .check()
            .is_err()
    );
    assert!(
        with(vec![cookie("a", "x.test", "/", "Sometimes")], vec![])
            .check()
            .is_err()
    );
    assert!(
        with(vec![cookie("a\n", "x.test", "/", "Lax")], vec![])
            .check()
            .is_err()
    );
    let many = vec![cookie("a", "x.test", "/", "Lax"); MAX_COOKIES + 1];
    assert!(with(many, vec![]).check().is_err());
    for origin in [
        "javascript:alert(1)",
        "file:///C:/",
        "https://x.test/path",
        "https://user:pw@x.test",
        "rbrowser://ui",
    ] {
        let origins = vec![OriginState {
            origin: origin.to_owned(),
            local_storage: Vec::new(),
        }];
        assert!(with(vec![], origins).check().is_err(), "{origin}");
    }
    assert_eq!(
        with(
            vec![cookie("a", "x.test", "/", "Strict")],
            vec![OriginState {
                origin: "http://localhost:8080".to_owned(),
                local_storage: Vec::new(),
            }]
        )
        .check(),
        Ok(())
    );
}

#[test]
fn the_export_keeps_the_environments_sites_only() {
    let config: rb_config::ProjectConfig = serde_json::from_value(json!({
        "version": 1,
        "startUrl": "http://localhost:8080/",
        "environments": [{ "id": "staging", "url": "https://staging.example.test/shop/" }]
    }))
    .expect("config");
    let hosts = scope_hosts(&config, &["https://tracker.example.org".to_owned()]);
    assert_eq!(hosts, ["localhost", "staging.example.test"]);
    let mut state = StorageState::from_chrome(
        &cookies_of(&json!({ "cookies": [
            { "name": "a", "value": "1", "domain": "staging.example.test", "path": "/" },
            { "name": "b", "value": "2", "domain": ".example.test", "path": "/" },
            { "name": "c", "value": "3", "domain": "api.staging.example.test", "path": "/" },
            { "name": "d", "value": "4", "domain": ".tracker.example.org", "path": "/" },
            { "name": "e", "value": "5", "domain": "localhost", "path": "/" },
            { "name": "f", "value": "6", "domain": "evil-staging.example.test", "path": "/" }
        ]})),
        vec![
            OriginState {
                origin: "https://staging.example.test".to_owned(),
                local_storage: Vec::new(),
            },
            OriginState {
                origin: "https://tracker.example.org".to_owned(),
                local_storage: Vec::new(),
            },
        ],
    );
    state.keep_hosts(&hosts);
    let names: Vec<&str> = state.cookies.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["a", "b", "c", "e"]);
    assert_eq!(state.origins.len(), 1);
    assert_eq!(state.origins[0].origin, "https://staging.example.test");
}

#[test]
fn without_environments_the_sites_shown_are_the_scope() {
    let config: rb_config::ProjectConfig =
        serde_json::from_value(json!({ "version": 1 })).expect("config");
    assert_eq!(
        scope_hosts(&config, &["https://app.example.test".to_owned()]),
        ["app.example.test"]
    );
}
