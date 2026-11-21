use rb_engine::ViewId;
use serde_json::json;

use super::{LogKind, LogLimits, LogStore, MAX_TEXT};

const VIEW: ViewId = ViewId(1);

#[test]
fn console_calls_and_browser_entries_become_messages() {
    let store = LogStore::default();
    store.record(
        VIEW,
        "Runtime.consoleAPICalled",
        &json!({
            "type": "warning",
            "args": [{ "type": "string", "value": "low" }, { "type": "number", "value": 3 },
                     { "type": "object", "description": "Object" }],
            "timestamp": 1000.5,
            "stackTrace": { "callFrames": [{ "url": "https://a.test/app.js", "lineNumber": 9 }] },
        }),
    );
    store.record(
        VIEW,
        "Log.entryAdded",
        &json!({ "entry": { "source": "security", "level": "error", "text": "mixed", "timestamp": 2000.0 } }),
    );
    let messages = store.console(VIEW, 10);
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].text, "low 3 Object");
    assert_eq!(messages[0].level, "warning");
    assert_eq!(messages[0].line, Some(10));
    assert_eq!(messages[1].source, "security");
}

#[test]
fn a_request_collects_its_response_and_end_but_no_headers() {
    let store = LogStore::default();
    store.record(
        VIEW,
        "Network.requestWillBeSent",
        &json!({
            "requestId": "r1",
            "request": { "url": "https://a.test/api", "method": "POST",
                         "headers": { "Authorization": "Bearer secret" }, "postData": "pw=1" },
            "type": "Fetch", "timestamp": 10.0, "wallTime": 1700000000.0,
        }),
    );
    store.record(
        VIEW,
        "Network.responseReceived",
        &json!({ "requestId": "r1", "type": "Fetch",
                 "response": { "status": 201, "mimeType": "application/json",
                               "headers": { "Set-Cookie": "sid=1" } } }),
    );
    store.record(
        VIEW,
        "Network.loadingFinished",
        &json!({ "requestId": "r1", "timestamp": 10.25, "encodedDataLength": 512.0 }),
    );
    let requests = store.network(VIEW, 10);
    assert_eq!(requests.len(), 1);
    let request = &requests[0];
    assert_eq!(request.status, Some(201));
    assert_eq!(request.duration_ms, Some(250.0));
    assert_eq!(request.size, Some(512.0));
    let wire = serde_json::to_string(request).expect("JSON");
    assert!(
        !wire.contains("secret") && !wire.contains("sid=1") && !wire.contains("pw=1"),
        "{wire}"
    );
}

#[test]
fn a_failed_request_and_an_exception_are_kept() {
    let store = LogStore::default();
    store.record(
        VIEW,
        "Network.requestWillBeSent",
        &json!({ "requestId": "r2", "request": { "url": "https://a.test/x", "method": "GET" },
                 "timestamp": 1.0, "wallTime": 1.0 }),
    );
    store.record(
        VIEW,
        "Network.loadingFailed",
        &json!({ "requestId": "r2", "errorText": "net::ERR_FAILED", "timestamp": 2.0 }),
    );
    assert_eq!(
        store.network(VIEW, 5)[0].failed.as_deref(),
        Some("net::ERR_FAILED")
    );
    store.record(
        VIEW,
        "Runtime.exceptionThrown",
        &json!({ "timestamp": 5.0, "exceptionDetails": {
            "text": "Uncaught", "url": "https://a.test/app.js", "lineNumber": 0, "columnNumber": 4,
            "exception": { "description": "TypeError: x is undefined" } } }),
    );
    let errors = store.errors(VIEW, 5);
    assert_eq!(errors[0].text, "Uncaught TypeError: x is undefined");
    assert_eq!((errors[0].line, errors[0].column), (Some(1), Some(5)));
}

#[test]
fn the_buffers_keep_only_the_newest_and_clear_and_forget() {
    let store = LogStore::new(LogLimits {
        console: 2,
        network: 1,
        errors: 1,
    });
    for text in ["a", "b", "c"] {
        store.record(
            VIEW,
            "Runtime.consoleAPICalled",
            &json!({ "type": "log", "args": [{ "type": "string", "value": text }] }),
        );
    }
    let texts: Vec<String> = store
        .console(VIEW, 10)
        .into_iter()
        .map(|m| m.text)
        .collect();
    assert_eq!(texts, vec!["b", "c"]);
    assert_eq!(store.console(VIEW, 1).len(), 1);
    store.clear(VIEW, LogKind::Console);
    assert!(store.console(VIEW, 10).is_empty());
    store.record(
        VIEW,
        "Runtime.exceptionThrown",
        &json!({ "exceptionDetails": {} }),
    );
    store.forget(VIEW);
    assert!(store.errors(VIEW, 10).is_empty());
    assert!(store.console(ViewId(9), 10).is_empty());
}

#[test]
fn long_texts_are_cut() {
    let store = LogStore::default();
    let long = "x".repeat(MAX_TEXT + 100);
    store.record(
        VIEW,
        "Runtime.consoleAPICalled",
        &json!({ "type": "log", "args": [{ "type": "string", "value": long }] }),
    );
    assert_eq!(store.console(VIEW, 1)[0].text.chars().count(), MAX_TEXT + 3);
}

#[test]
fn credentials_in_texts_and_urls_never_reach_the_buffers() {
    // Review L1 of Charge 6: the buffers feed MCP and the plugins' `$console`.
    let store = LogStore::default();
    store.record(
        VIEW,
        "Runtime.consoleAPICalled",
        &json!({
            "type": "log",
            "args": [{ "type": "string", "value": "login ok token=abc123 Bearer ey.J" }],
            "stackTrace": { "callFrames": [{ "url": "https://a.test/cb?code=4/0Ab&x=1" }] },
        }),
    );
    store.record(
        VIEW,
        "Log.entryAdded",
        &json!({ "entry": { "text": "GET https://u:pw@a.test/", "url": "https://a.test/?sig=s1" } }),
    );
    store.record(
        VIEW,
        "Runtime.exceptionThrown",
        &json!({ "exceptionDetails": {
            "text": "Uncaught", "url": "https://a.test/#access_token=t0&expires_in=9"
        } }),
    );
    store.record(
        VIEW,
        "Network.requestWillBeSent",
        &json!({ "requestId": "r1", "request": {
            "url": "https://a.test/api?api_key=k9&X-Amz-Signature=f00", "method": "GET"
        } }),
    );
    let console = store.console(VIEW, 10);
    assert_eq!(console[0].text, "login ok token=*** Bearer ***");
    assert_eq!(
        console[0].url.as_deref(),
        Some("https://a.test/cb?code=***&x=1")
    );
    assert_eq!(console[1].text, "GET https://***@a.test/");
    assert_eq!(console[1].url.as_deref(), Some("https://a.test/?sig=***"));
    let errors = store.errors(VIEW, 10);
    assert_eq!(
        errors[0].url.as_deref(),
        Some("https://a.test/#access_token=***&expires_in=9")
    );
    let network = store.network(VIEW, 10);
    assert_eq!(
        network[0].url,
        "https://a.test/api?api_key=***&X-Amz-Signature=***"
    );
}

#[test]
fn audits_issues_of_csp_mixed_content_and_deprecation_are_kept_masked() {
    // RB-080-09: the error report needs the issues Chromium no longer logs to the console.
    let store = LogStore::default();
    store.record(
        VIEW,
        "Audits.issueAdded",
        &json!({ "issue": { "code": "ContentSecurityPolicyIssue", "details": {
            "contentSecurityPolicyIssueDetails": {
                "blockedURL": "https://cdn.test/pixel.png?token=t1",
                "violatedDirective": "img-src", "isReportOnly": false,
                "contentSecurityPolicyViolationType": "kURLViolation",
                "sourceCodeLocation": { "url": "https://a.test/", "lineNumber": 4, "columnNumber": 0 } } } } }),
    );
    store.record(
        VIEW,
        "Audits.issueAdded",
        &json!({ "issue": { "code": "MixedContentIssue", "details": {
            "mixedContentIssueDetails": {
                "resourceType": "Script", "resolutionStatus": "MixedContentBlocked",
                "insecureURL": "http://a.test/mixed.js", "mainResourceURL": "https://a.test/" } } } }),
    );
    store.record(
        VIEW,
        "Audits.issueAdded",
        &json!({ "issue": { "code": "DeprecationIssue", "details": {
            "deprecationIssueDetails": { "type": "XHRSynchronous",
                "sourceCodeLocation": { "url": "https://a.test/app.js", "lineNumber": 9, "columnNumber": 2 } } } } }),
    );
    store.record(
        VIEW,
        "Audits.issueAdded",
        &json!({ "issue": { "code": "CookieIssue", "details": {} } }),
    );
    let issues = store.issues(VIEW, 10);
    assert_eq!(issues.len(), 3);
    assert_eq!(issues[0].code, "ContentSecurityPolicyIssue");
    assert_eq!(issues[0].kind.as_deref(), Some("img-src"));
    assert_eq!(issues[0].detail.as_deref(), Some("kURLViolation"));
    assert_eq!(
        issues[0].url.as_deref(),
        Some("https://cdn.test/pixel.png?token=***")
    );
    assert_eq!((issues[0].line, issues[0].column), (Some(5), Some(1)));
    assert!(!issues[0].report_only);
    assert!(issues[0].time > 0.0);
    assert_eq!(issues[1].kind.as_deref(), Some("MixedContentBlocked"));
    assert_eq!(issues[1].detail.as_deref(), Some("Script"));
    assert_eq!(issues[1].url.as_deref(), Some("http://a.test/mixed.js"));
    assert_eq!(issues[1].source_url.as_deref(), Some("https://a.test/"));
    assert_eq!(issues[2].kind.as_deref(), Some("XHRSynchronous"));
    assert_eq!(issues[2].url, None);
    assert_eq!(issues[2].line, Some(10));
    let wire = serde_json::to_value(&issues[2]).expect("JSON");
    assert_eq!(wire["sourceUrl"], "https://a.test/app.js");
    assert!(wire.get("reportOnly").is_none());
    assert!(super::DOMAINS.contains(&"Audits.enable"));
    store.forget(VIEW);
    assert!(store.issues(VIEW, 10).is_empty());
}

#[test]
fn a_new_document_marks_the_newest_time_of_the_old_one() {
    let store = LogStore::default();
    assert_eq!(store.navigated_at(VIEW), None);
    store.record(
        VIEW,
        "Runtime.consoleAPICalled",
        &json!({ "type": "log", "args": [], "timestamp": 1000.0 }),
    );
    store.record(
        VIEW,
        "Runtime.exceptionThrown",
        &json!({ "timestamp": 1500.0, "exceptionDetails": { "text": "boom" } }),
    );
    // Before any navigation nothing is marked.
    assert_eq!(store.navigated_at(VIEW), None);
    store.record(VIEW, "Runtime.executionContextsCleared", &json!({}));
    assert_eq!(store.navigated_at(VIEW), Some(1500.0));
    // A further clear without anything new keeps the mark; a later message moves the next one.
    store.record(VIEW, "Runtime.executionContextsCleared", &json!({}));
    assert_eq!(store.navigated_at(VIEW), Some(1500.0));
    store.record(
        VIEW,
        "Runtime.consoleAPICalled",
        &json!({ "type": "log", "args": [], "timestamp": 2000.0 }),
    );
    assert_eq!(store.navigated_at(VIEW), Some(1500.0));
    store.record(VIEW, "Runtime.executionContextsCleared", &json!({}));
    assert_eq!(store.navigated_at(VIEW), Some(2000.0));
    assert_eq!(store.navigated_at(ViewId(2)), None);
}

#[test]
fn a_request_keeps_its_headers_for_a_repeat_off_the_wire() {
    let store = LogStore::default();
    store.record(
        VIEW,
        "Network.requestWillBeSent",
        &json!({ "requestId": "r1", "request": { "url": "https://a.test/api", "method": "POST",
                 "headers": { "Content-Type": "application/json", "Authorization": "Bearer secret" } },
                 "timestamp": 1.0, "wallTime": 1.0 }),
    );
    let request = store.request(VIEW, "r1").expect("recorded");
    assert_eq!(request.method, "POST");
    let mut headers = request.request_headers().to_vec();
    headers.sort();
    assert_eq!(
        headers,
        [
            ("Authorization".to_owned(), "Bearer secret".to_owned()),
            ("Content-Type".to_owned(), "application/json".to_owned()),
        ]
    );
    assert!(store.request(VIEW, "r9").is_none());
    assert!(
        !serde_json::to_string(&request)
            .expect("JSON")
            .contains("secret")
    );
}

#[test]
fn a_response_keeps_its_waiting_time_caching_and_compression_and_the_decoded_size() {
    let store = LogStore::default();
    store.record(
        VIEW,
        "Network.requestWillBeSent",
        &json!({ "requestId": "r1", "request": { "url": "https://a.test/api/users/1", "method": "GET" },
                 "type": "Fetch", "timestamp": 1.0, "wallTime": 1.0 }),
    );
    store.record(
        VIEW,
        "Network.responseReceived",
        &json!({ "requestId": "r1", "type": "Fetch",
                 "response": { "status": 200, "mimeType": "application/json",
                               "timing": { "sendEnd": 2.0, "receiveHeadersEnd": 52.0 },
                               "headers": { "content-encoding": "gzip", "Cache-Control": "no-cache",
                                            "Set-Cookie": "sid=secret" } } }),
    );
    for length in [1000.0, 2000.5] {
        store.record(
            VIEW,
            "Network.dataReceived",
            &json!({ "requestId": "r1", "dataLength": length, "encodedDataLength": 10.0 }),
        );
    }
    let request = store.request(VIEW, "r1").expect("recorded");
    let response = request.response.as_ref().expect("the response facts");
    assert_eq!(response.ttfb_ms, Some(50.0));
    assert_eq!(response.encoding.as_deref(), Some("gzip"));
    assert_eq!(response.cache_control.as_deref(), Some("no-cache"));
    assert_eq!(request.decoded_size, Some(3000.5));
    let wire = serde_json::to_value(&request).expect("JSON");
    assert_eq!(wire["response"]["ttfbMs"], json!(50.0));
    assert_eq!(wire["decodedSize"], json!(3000.5));
    assert!(!wire.to_string().contains("secret"), "{wire}");
}

#[test]
fn a_request_without_a_response_carries_no_response_facts() {
    let store = LogStore::default();
    store.record(
        VIEW,
        "Network.requestWillBeSent",
        &json!({ "requestId": "r1", "request": { "url": "https://a.test/", "method": "GET" },
                 "timestamp": 1.0, "wallTime": 1.0 }),
    );
    let wire = serde_json::to_value(store.request(VIEW, "r1").expect("recorded")).expect("JSON");
    assert!(
        wire.get("response").is_none() && wire.get("decodedSize").is_none(),
        "{wire}"
    );
}

/// RB-090-34: `Network` is not collected until someone asks for the requests (or
/// `mcp.collectNetwork` wants it); the first read switches it on, once, for good.
#[test]
fn network_collection_starts_with_the_first_read_of_the_requests() {
    let store = LogStore::default();
    let mut wanted = store.network_wanted();
    assert!(!*wanted.borrow_and_update(), "off at the start");
    assert!(!super::DOMAINS.contains(&super::NETWORK_DOMAIN));
    assert!(store.console(VIEW, 10).is_empty());
    assert!(store.errors(VIEW, 10).is_empty());
    assert!(
        !wanted.has_changed().expect("open"),
        "other reads leave it off"
    );
    assert!(store.network(VIEW, 10).is_empty(), "nothing collected yet");
    assert!(
        wanted.has_changed().expect("open"),
        "the read switched it on"
    );
    assert!(*wanted.borrow_and_update());
    assert!(store.request(VIEW, "r1").is_none());
    store.want_network();
    assert!(
        !wanted.has_changed().expect("open"),
        "switched on once, never again"
    );
}

#[test]
fn want_network_switches_collection_on_without_a_read() {
    let store = LogStore::default();
    store.want_network();
    assert!(*store.network_wanted().borrow());
}
