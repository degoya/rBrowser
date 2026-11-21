use serde_json::{Value, json};

use super::{Call, calls, compressible};

fn feed(views: Value) -> Value {
    json!({ "$console": { "views": views } })
}

#[test]
fn every_instance_of_every_view_is_a_call_with_its_response_facts() {
    let args = feed(json!([
        { "view": 1, "navigatedAt": 100.0, "network": [
            { "id": "1.1", "url": "https://a.test/api/users/1", "method": "GET", "resourceType": "Fetch",
              "status": 200, "mimeType": "application/json", "fromCache": false, "size": 300.0,
              "decodedSize": 1200.0, "time": 90.0, "durationMs": 40.0 },
            { "id": "1.2", "url": "https://a.test/api/users/1", "method": "GET", "resourceType": "Fetch",
              "status": 200, "fromCache": true, "time": 150.0, "durationMs": 2.0,
              "response": { "ttfbMs": 30.0, "encoding": "br", "cacheControl": "max-age=60",
                            "expires": false, "etag": true, "lastModified": false } },
        ] },
        { "view": 2, "network": [
            { "id": "2.1", "url": "https://a.test/x", "method": "POST", "time": 5.0, "failed": "net::ERR_FAILED" },
            { "url": "https://a.test/no-method" },
        ] },
        { "view": "bad", "network": [{ "id": "x", "url": "https://a.test/", "method": "GET" }] },
    ]));
    let found = calls(&args);
    assert_eq!(found.len(), 3);
    let first = &found[0];
    assert_eq!(first.view, 1);
    assert_eq!(first.resource_type, "Fetch");
    assert_eq!(first.decoded_size, Some(1200.0));
    assert!(first.response.is_none());
    // Before the view's last new document: not the current page's.
    assert!(!first.current);
    let second = &found[1];
    assert!(second.current && second.from_cache);
    let response = second.response.as_ref().expect("facts");
    assert_eq!(response.ttfb_ms, Some(30.0));
    assert_eq!(response.encoding.as_deref(), Some("br"));
    assert!(response.etag && !response.expires);
    let third: &Call = &found[2];
    assert_eq!(third.resource_type, "Other");
    assert!(third.failed && third.current);
}

#[test]
fn no_feed_is_no_calls() {
    assert!(calls(&json!({})).is_empty());
}

#[test]
fn text_json_scripts_and_svg_compress_images_do_not() {
    for mime in [
        "application/json",
        "text/html",
        "application/javascript",
        "image/svg+xml",
        "application/problem+json",
        "application/xml",
    ] {
        assert!(compressible(Some(mime)), "{mime}");
    }
    for mime in ["image/png", "font/woff2", "application/octet-stream"] {
        assert!(!compressible(Some(mime)), "{mime}");
    }
    assert!(!compressible(None));
}
