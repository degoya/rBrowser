use serde_json::json;

use super::*;

fn masker() -> Masker {
    Masker::new(["cookie-value-4713"])
}

/// Two views, the same error in both, a warning, a log line, an exception and requests - one
/// failed twice, one aborted, one fine - with fields no report may carry.
fn feed() -> Value {
    json!({ "views": [
        { "view": 1,
          "console": [
            { "level": "error", "source": "console-api", "text": "Boom 1", "url": "https://example.test/app.js?v=3", "line": 12, "time": 1.0 },
            { "level": "warning", "source": "console-api", "text": "Deprecated", "time": 2.0 },
            { "level": "log", "source": "console-api", "text": "hello", "time": 3.0 },
            { "level": "error", "source": "console-api", "text": "sid=cookie-value-4713", "time": 4.0 }
          ],
          "errors": [
            { "text": "Uncaught TypeError: x is undefined", "url": "https://example.test/app.js", "line": 3, "column": 9, "time": 5.0 }
          ],
          "network": [
            { "id": "1", "url": "https://example.test/api?token=q-token-4711", "method": "GET", "status": 401, "fromCache": false, "time": 6.0,
              "requestHeaders": { "Authorization": "Bearer auth-value-4712" }, "postData": "password=hunter22" },
            { "id": "2", "url": "https://example.test/ok", "method": "GET", "status": 200, "fromCache": false, "time": 7.0 },
            { "id": "3", "url": "https://example.test/slow", "method": "POST", "failed": "net::ERR_ABORTED", "fromCache": false, "time": 8.0 }
          ] },
        { "view": 2,
          "console": [
            { "level": "error", "source": "console-api", "text": "Boom 1", "url": "https://example.test/app.js?v=3", "line": 12, "time": 1.5 }
          ],
          "errors": [],
          "network": [
            { "id": "9", "url": "https://example.test/api?token=other", "method": "GET", "status": 401, "fromCache": false, "time": 6.5 }
          ] },
        { "view": 7, "console": [ { "level": "error", "text": "not in the report" } ] }
    ] })
}

#[test]
fn errors_and_warnings_are_deduplicated_over_the_views() {
    let problems = collect(&feed(), &[1, 2], &masker());
    let texts: Vec<&str> = problems
        .console
        .iter()
        .map(|line| line.text.as_str())
        .collect();
    assert_eq!(
        texts,
        [
            "Boom 1",
            "Deprecated",
            "sid=***",
            "Uncaught TypeError: x is undefined"
        ]
    );
    let boom = &problems.console[0];
    assert_eq!(boom.count, 2);
    assert_eq!(boom.views.iter().copied().collect::<Vec<_>>(), [1, 2]);
    assert_eq!(boom.level, Level::Error);
    assert_eq!(
        boom.source.as_deref(),
        Some("https://example.test/app.js?v=***:12")
    );
    assert_eq!(problems.console[1].level, Level::Warning);
    assert!(!texts.contains(&"hello"), "a log line is no problem");
    assert!(
        !texts.contains(&"not in the report"),
        "only the report's views"
    );
}

#[test]
fn failed_requests_are_400_and_above_or_aborted_and_masked() {
    let problems = collect(&feed(), &[1, 2], &masker());
    assert_eq!(problems.requests.len(), 2);
    let unauthorized = &problems.requests[0];
    assert_eq!(unauthorized.url, "https://example.test/api?token=***");
    assert_eq!(unauthorized.status, Some(401));
    assert_eq!(
        unauthorized.count, 2,
        "the masked addresses are one request"
    );
    let aborted = &problems.requests[1];
    assert_eq!(aborted.method, "POST");
    assert_eq!(aborted.failure.as_deref(), Some("net::ERR_ABORTED"));
}

#[test]
fn headers_and_bodies_never_reach_the_problems() {
    let problems = collect(&feed(), &[1, 2], &masker());
    let shown = format!("{problems:?}");
    for secret in [
        "q-token-4711",
        "auth-value-4712",
        "cookie-value-4713",
        "hunter22",
        "Authorization",
    ] {
        assert!(!shown.contains(secret), "{secret} in {shown}");
    }
}

#[test]
fn hostile_entries_are_bounded_or_skipped() {
    let long = "x".repeat(MAX_TEXT_CHARS * 2);
    let mut console: Vec<Value> = (0..MAX_ITEMS + 50)
        .map(|n| json!({ "level": "error", "text": format!("e{n}") }))
        .collect();
    console.push(json!({ "level": "error", "text": long }));
    console.push(json!({ "level": "error", "text": 5 }));
    console.push(json!("no object"));
    let feed = json!({ "views": [
        { "view": 1, "console": console, "network": [
            { "url": "https://example.test/a", "method": "G\nET <script>", "status": 500 },
            { "url": 3, "status": 500 }
        ] },
        { "view": "1" },
        "nothing"
    ] });
    let problems = collect(&feed, &[1], &masker());
    assert_eq!(problems.console.len(), MAX_ITEMS);
    assert!(problems.dropped > 0);
    assert!(
        problems
            .console
            .iter()
            .all(|line| line.text.chars().count() <= MAX_TEXT_CHARS + 3)
    );
    assert_eq!(problems.requests.len(), 1);
    assert_eq!(problems.requests[0].method, "?", "only a token of letters");
    assert_eq!(collect(&json!({}), &[1], &masker()), Problems::default());
}
