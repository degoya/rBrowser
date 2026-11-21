use serde_json::json;

use super::{
    Capture, MAX_COOKIES, MAX_KEYS, MAX_REQUESTS, Phase, banner_of, cookies_of, keys_of, request_of,
};

fn sent(url: &str, kind: &str) -> serde_json::Value {
    json!({ "requestId": "1", "request": { "url": url, "method": "GET",
        "headers": { "Cookie": "sid=secret" } }, "type": kind })
}

#[test]
fn a_request_keeps_its_url_and_kind_only() {
    assert_eq!(
        request_of(&sent("https://cdn.example.test/a.js", "Script")),
        Some(json!({ "url": "https://cdn.example.test/a.js", "resourceType": "Script" }))
    );
    assert_eq!(
        request_of(&json!({ "request": { "url": "https://example.test/" } })),
        Some(json!({ "url": "https://example.test/", "resourceType": "Other" }))
    );
    // No host to judge: not a request of the capture.
    assert_eq!(
        request_of(&sent("data:image/png;base64,AAAA", "Image")),
        None
    );
    assert_eq!(
        request_of(&sent("blob:https://example.test/1", "Fetch")),
        None
    );
    assert_eq!(request_of(&json!({})), None);
}

#[test]
fn a_request_url_is_masked_as_the_console_feed_is() {
    let seen = request_of(&sent(
        "https://track.example.test/p?access_token=abc123&x=1",
        "XHR",
    ))
    .expect("request");
    let url = seen["url"].as_str().expect("url");
    assert!(!url.contains("abc123"), "{url}");
    assert!(url.starts_with("https://track.example.test/p?"));
}

#[test]
fn a_cookie_never_carries_its_value() {
    let answer = json!({ "cookies": [
        { "name": "_ga", "value": "GA1.2.secret", "domain": ".example.test", "path": "/",
          "expires": 1821536000.5, "size": 30, "httpOnly": false, "secure": true,
          "session": false, "sameSite": "Lax", "partitionKey": { "topLevelSite": "x" } },
        { "name": "sid", "value": "hunter2", "domain": "example.test", "expires": -1,
          "session": true },
        { "value": "nameless", "domain": "example.test" },
    ] });
    let cookies = cookies_of(&answer);
    assert_eq!(
        cookies,
        vec![
            json!({ "name": "_ga", "domain": ".example.test", "expires": 1821536000.5,
                    "session": false }),
            json!({ "name": "sid", "domain": "example.test", "expires": null,
                    "session": true }),
        ]
    );
    let text = serde_json::to_string(&cookies).expect("json");
    for secret in [
        "GA1.2.secret",
        "hunter2",
        "nameless",
        "httpOnly",
        "partitionKey",
    ] {
        assert!(!text.contains(secret), "{secret} in {text}");
    }
}

#[test]
fn the_lists_are_bounded() {
    let many: Vec<_> = (0..MAX_COOKIES + 5)
        .map(|n| json!({ "name": format!("c{n}"), "domain": "example.test", "expires": -1 }))
        .collect();
    assert_eq!(cookies_of(&json!({ "cookies": many })).len(), MAX_COOKIES);
    let keys: Vec<_> = (0..MAX_KEYS + 5).map(|n| format!("k{n}")).collect();
    assert_eq!(keys_of(&json!({ "storage": keys })).len(), MAX_KEYS);
    let mut phase = Phase::default();
    for n in 0..MAX_REQUESTS + 5 {
        phase.add_request(&sent(&format!("https://example.test/{n}"), "Image"));
    }
    assert_eq!(phase.requests.len(), MAX_REQUESTS);
    assert!(phase.requests_cut);
}

#[test]
fn storage_keys_are_names_only() {
    assert_eq!(
        keys_of(&json!({ "storage": ["consent", "", 3, "theme"] })),
        vec!["consent".to_owned(), "theme".to_owned()]
    );
    assert!(keys_of(&json!({})).is_empty());
}

#[test]
fn the_banner_comes_from_the_read_and_the_click() {
    let read = json!({ "banner": { "cmp": "onetrust", "visible": true, "rejectable": true } });
    assert_eq!(
        banner_of(&read, Some(&json!({ "clicked": true }))),
        json!({ "cmp": "onetrust", "visible": true, "rejected": true })
    );
    assert_eq!(
        banner_of(&read, None),
        json!({ "cmp": "onetrust", "visible": true, "rejected": false })
    );
    assert_eq!(
        banner_of(&json!({}), Some(&json!({ "clicked": "yes" }))),
        json!({ "cmp": null, "visible": false, "rejected": false })
    );
}

#[test]
fn the_capture_has_the_plugins_shape() {
    let mut before = Phase::default();
    before.add_request(&sent("https://cdn.example.test/a.js", "Script"));
    before.cookies = cookies_of(&json!({ "cookies": [
        { "name": "_ga", "value": "v", "domain": ".example.test", "expires": 1821536000 }
    ] }));
    before.storage = vec!["consent".to_owned()];
    let capture = Capture {
        url: "https://example.test/".to_owned(),
        captured_at: 1_790_000_000,
        fresh: true,
        discarded: true,
        banner: json!({ "cmp": "onetrust", "visible": true, "rejected": true }),
        before,
        after_reject: Some(Phase::default()),
    };
    assert_eq!(
        capture.to_value(),
        json!({
            "url": "https://example.test/",
            "capturedAt": 1_790_000_000,
            "session": { "fresh": true, "discarded": true },
            "banner": { "cmp": "onetrust", "visible": true, "rejected": true },
            "before": {
                "requests": [{ "url": "https://cdn.example.test/a.js", "resourceType": "Script" }],
                "requestsCut": false,
                "cookies": [{ "name": "_ga", "domain": ".example.test", "expires": 1821536000.0,
                              "session": false }],
                "storage": ["consent"],
            },
            "afterReject": { "requests": [], "requestsCut": false, "cookies": [], "storage": [] },
        })
    );
    let without = Capture {
        after_reject: None,
        ..capture
    };
    assert!(without.to_value().get("afterReject").is_none());
}
