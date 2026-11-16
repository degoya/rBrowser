use serde_json::json;

use super::*;
use crate::network_rules::rule::UrlPattern;

#[test]
fn breakpoints_read_with_stage_views_and_switch() {
    let (breakpoints, rejected) = parse_breakpoints(&json!([
        { "id": "bp-1", "match": { "urlPattern": "*/api/*", "method": "POST" }, "stage": "response",
          "views": [1, 2], "enabled": false },
        { "id": "bp-2", "match": { "urlPattern": "/x/", "regex": true } },
        { "id": "bad", "match": { "urlPattern": "*" }, "stage": "sideways" },
        { "id": "bp-1", "match": { "urlPattern": "*" } },
    ]));
    assert_eq!(breakpoints.len(), 2);
    let first = &breakpoints[0];
    assert_eq!(first.id, "bp-1");
    assert_eq!(first.stage, Stage::Response);
    assert_eq!(first.views, Some(vec![ViewId(1), ViewId(2)]));
    assert!(!first.enabled);
    assert_eq!(first.matcher.method.as_deref(), Some("POST"));
    let second = &breakpoints[1];
    assert_eq!(second.stage, Stage::Request);
    assert_eq!(second.views, None);
    assert!(second.enabled);
    assert!(matches!(second.matcher.url, UrlPattern::Regex(_)));
    assert_eq!(
        rejected
            .iter()
            .map(|r| (r.id.as_str(), r.reason.as_str()))
            .collect::<Vec<_>>(),
        [("bad", "bad stage"), ("bp-1", "duplicate id")]
    );
}

#[test]
fn the_settings_read_with_their_defaults_and_bounds() {
    assert_eq!(parse_settings(&Value::Null), DebugSettings::default());
    let settings = parse_settings(&json!({
        "breakpointTimeoutMs": 5000, "maxBodyBytes": 1024, "mapLocalRoots": ["C:\\sites", 7, ""],
        "allowComposeFromMcp": true,
    }));
    assert_eq!(settings.timeout_ms, 5000);
    assert_eq!(settings.max_body_bytes, 1024);
    assert_eq!(settings.map_local_roots, vec!["C:\\sites".to_owned()]);
    assert!(settings.allow_compose_from_mcp);
    let capped =
        parse_settings(&json!({ "breakpointTimeoutMs": 10_000_000, "maxBodyBytes": 1_u64 << 40 }));
    assert_eq!(capped.timeout_ms, MAX_TIMEOUT_MS);
    assert_eq!(capped.max_body_bytes, MAX_BODY_BYTES);
    assert_eq!(
        parse_settings(&json!({ "breakpointTimeoutMs": 0 })).timeout_ms,
        0
    );
}

#[test]
fn asks_read_continue_abort_compose_and_repeat() {
    let (asks, rejected) = parse_asks(&json!([
        { "id": "a1", "kind": "continue", "held": "h3", "allViews": true,
          "edits": { "method": "PUT", "url": "https://a.test/x", "headers": [["x-a", "1"]],
                     "body": "hello", "status": 503 } },
        { "id": "a2", "kind": "abort", "held": "h4" },
        { "id": "a3", "kind": "compose", "view": 1, "times": 3,
          "request": { "method": "post", "url": "https://a.test/api", "headers": [["content-type", "text/plain"]],
                       "body": "hi" } },
        { "id": "a4", "kind": "repeat", "view": 2, "requestId": "12.7", "edits": { "body": "x" } },
        { "id": "a5", "kind": "dance" },
        { "id": "a6", "kind": "continue" },
        { "id": "a7", "kind": "compose", "request": { "method": "GET", "url": "ftp://x" } },
        { "kind": "abort", "held": "h1" },
    ]));
    assert_eq!(asks.len(), 4);
    let Ask::Continue {
        id,
        held,
        all_views,
        edits,
    } = &asks[0]
    else {
        panic!("a continue");
    };
    assert_eq!((id.as_str(), held.as_str(), *all_views), ("a1", "h3", true));
    assert_eq!(edits.method.as_deref(), Some("PUT"));
    assert_eq!(edits.url.as_deref(), Some("https://a.test/x"));
    assert_eq!(
        edits.headers,
        Some(vec![("x-a".to_owned(), "1".to_owned())])
    );
    assert_eq!(edits.body.as_deref(), Some(b"hello".as_slice()));
    assert_eq!(edits.status, Some(503));
    assert!(
        matches!(&asks[1], Ask::Abort { id, held, all_views } if id == "a2" && held == "h4" && !all_views)
    );
    let Ask::Compose {
        id,
        view,
        request,
        times,
    } = &asks[2]
    else {
        panic!("a compose");
    };
    assert_eq!((id.as_str(), *view, *times), ("a3", Some(ViewId(1)), 3));
    assert_eq!(request.method, "POST");
    assert_eq!(request.body.as_deref(), Some(b"hi".as_slice()));
    let Ask::Repeat {
        id,
        view,
        request_id,
        edits,
        times,
    } = &asks[3]
    else {
        panic!("a repeat");
    };
    assert_eq!(
        (id.as_str(), *view, request_id.as_str(), *times),
        ("a4", ViewId(2), "12.7", 1)
    );
    assert_eq!(edits.body.as_deref(), Some(b"x".as_slice()));
    assert_eq!(
        rejected.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
        ["a5", "a6", "a7", "#7"]
    );
}

#[test]
fn an_edit_body_may_come_as_base64_and_times_are_capped() {
    let (asks, _) = parse_asks(&json!([
        { "id": "a1", "kind": "continue", "held": "h1", "edits": { "bodyBase64": "aGk=" } },
        { "id": "a2", "kind": "compose", "times": 99, "request": { "method": "GET", "url": "http://a.test/" } },
    ]));
    let Ask::Continue { edits, .. } = &asks[0] else {
        panic!("a continue");
    };
    assert_eq!(edits.body.as_deref(), Some(b"hi".as_slice()));
    assert!(matches!(&asks[1], Ask::Compose { times, .. } if *times == MAX_TIMES));
}
