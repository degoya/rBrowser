use rb_plugin_types::codes;
use serde_json::json;

use super::{
    ASK_KEY, MAX_SCRIPT_BYTES, PROBE_KEY, ProbeRequest, is_offered, offered, probe_url, requested,
    with_consent,
};

#[test]
fn a_caller_sent_capture_is_dropped_for_every_plugin() {
    let forged = json!({ PROBE_KEY: { "url": "https://example.test/" }, ASK_KEY: true, "x": 1 });
    assert_eq!(offered(&forged, false), json!({ "x": 1 }));
    assert_eq!(offered(&forged, true), json!({ ASK_KEY: true, "x": 1 }));
}

#[test]
fn only_a_granted_plugin_is_offered_the_probe() {
    assert!(is_offered(&offered(&json!({}), true)));
    assert!(!is_offered(&offered(&json!({}), false)));
    assert_eq!(offered(&json!(null), true), json!({ ASK_KEY: true }));
    assert_eq!(offered(&json!(null), false), json!(null));
    // Not an object: as `$console` leaves it.
    assert_eq!(offered(&json!([1]), true), json!([1]));
    // A caller's `"$consentProbe": "yes"` is no offer either.
    assert!(!is_offered(&json!({ ASK_KEY: "yes" })));
}

#[test]
fn the_second_call_carries_the_hosts_capture_and_no_offer() {
    let first = offered(&json!({ "url": "https://example.test/" }), true);
    let second = with_consent(&first, json!({ "url": "https://example.test/" }));
    assert_eq!(
        second,
        json!({ "url": "https://example.test/", PROBE_KEY: { "url": "https://example.test/" } })
    );
    assert!(!is_offered(&second));
    assert_eq!(
        with_consent(&json!(null), json!({ "a": 1 })),
        json!({ PROBE_KEY: { "a": 1 } })
    );
}

#[test]
fn an_answer_without_a_request_is_no_probe() {
    assert!(requested(r#"{"findings": []}"#).is_none());
    assert!(requested("not json").is_none());
    assert!(requested("[1]").is_none());
}

#[test]
fn a_request_is_read_with_its_scripts() {
    let answer = json!({ ASK_KEY: {
        "url": "https://example.test/start",
        "read": "(() => 1)()",
        "reject": "(() => 2)()",
    } })
    .to_string();
    assert_eq!(
        requested(&answer).expect("asked").expect("valid"),
        ProbeRequest {
            url: "https://example.test/start".to_owned(),
            read: "(() => 1)()".to_owned(),
            reject: Some("(() => 2)()".to_owned()),
        }
    );
    let no_reject =
        json!({ ASK_KEY: { "url": "http://example.test/", "read": "1", "reject": null } });
    let request = requested(&no_reject.to_string())
        .expect("asked")
        .expect("valid");
    assert_eq!(request.reject, None);
}

#[test]
fn a_malformed_request_fails_the_call() {
    let too_long = "x".repeat(MAX_SCRIPT_BYTES + 1);
    for bad in [
        json!({ ASK_KEY: true }),
        json!({ ASK_KEY: { "url": "https://example.test/" } }),
        json!({ ASK_KEY: { "url": "https://example.test/", "read": "" } }),
        json!({ ASK_KEY: { "url": "https://example.test/", "read": "1", "reject": 3 } }),
        json!({ ASK_KEY: { "url": "file:///etc/passwd", "read": "1" } }),
        json!({ ASK_KEY: { "url": "https://example.test/", "read": too_long } }),
    ] {
        let failure = requested(&bad.to_string())
            .expect("asked")
            .expect_err("refused");
        assert_eq!(failure.code(), Some(codes::INVALID), "{bad}");
    }
}

#[test]
fn only_plain_http_and_https_addresses_are_probed() {
    assert_eq!(
        probe_url("https://example.test/a?b=1#c").as_deref(),
        Ok("https://example.test/a?b=1#c")
    );
    assert_eq!(
        probe_url("http://example.test").as_deref(),
        Ok("http://example.test/")
    );
    for bad in [
        "",
        "example.test",
        "file:///C:/secret.txt",
        "javascript:alert(1)",
        "data:text/html,x",
        "chrome://settings",
        "about:blank",
        "rbrowser://ui/",
        "ftp://example.test/",
        "https://user:pass@example.test/",
        "https://user@example.test/",
        "https://example.test\\@evil.test/",
        " https://example.test/",
        "https://exa mple.test/",
        "https://example.test/\n",
    ] {
        assert!(probe_url(bad).is_err(), "{bad:?}");
    }
    let long = format!("https://example.test/{}", "a".repeat(2048));
    assert!(probe_url(&long).is_err());
}
