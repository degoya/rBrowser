use serde_json::json;

use super::{MAX_CACHE_CONTROL, ResponseFacts, facts};

#[test]
fn the_waiting_time_is_the_first_byte_after_the_request_went_out() {
    let response = json!({
        "status": 200,
        "timing": { "requestTime": 10.0, "sendStart": 2.0, "sendEnd": 3.5,
                    "receiveHeadersStart": 83.5, "receiveHeadersEnd": 90.0 },
        "headers": {},
    });
    assert_eq!(facts(&response).ttfb_ms, Some(80.0));
}

#[test]
fn without_a_header_start_the_header_end_counts() {
    let response = json!({
        "timing": { "sendEnd": 1.0, "receiveHeadersEnd": 41.0 },
    });
    assert_eq!(facts(&response).ttfb_ms, Some(40.0));
}

#[test]
fn a_response_from_a_cache_has_no_waiting_time() {
    let cached = json!({ "timing": { "sendEnd": -1.0, "receiveHeadersEnd": 2.0 } });
    assert_eq!(facts(&cached).ttfb_ms, None);
    assert_eq!(facts(&json!({})).ttfb_ms, None);
}

#[test]
fn the_caching_and_compression_headers_are_read_in_any_case() {
    let response = json!({
        "headers": {
            "Content-Encoding": "BR",
            "cache-control": "public, max-age=60",
            "ETag": "\"abc\"",
            "Expires": "Thu, 01 Dec 2026 16:00:00 GMT",
            "Set-Cookie": "sid=secret",
        },
    });
    let found = facts(&response);
    assert_eq!(found.encoding.as_deref(), Some("br"));
    assert_eq!(found.cache_control.as_deref(), Some("public, max-age=60"));
    assert!(found.etag && found.expires && !found.last_modified);
    let wire = serde_json::to_string(&found).expect("JSON");
    assert!(!wire.contains("secret"), "{wire}");
    assert!(wire.contains("\"cacheControl\""), "{wire}");
}

#[test]
fn no_headers_are_no_facts_and_identity_is_no_encoding() {
    assert_eq!(facts(&json!({ "headers": {} })), ResponseFacts::default());
    let identity = json!({ "headers": { "content-encoding": "identity", "last-modified": "x" } });
    let found = facts(&identity);
    assert_eq!(found.encoding, None);
    assert!(found.last_modified);
}

#[test]
fn a_long_cache_control_is_cut() {
    let long = "max-age=1, ".repeat(100);
    let found = facts(&json!({ "headers": { "Cache-Control": long } }));
    assert_eq!(
        found.cache_control.map(|text| text.chars().count()),
        Some(MAX_CACHE_CONTROL)
    );
}
