use serde_json::json;

use super::*;
use crate::feed::ViewFeed;

fn feed(value: serde_json::Value) -> ViewFeed {
    serde_json::from_value(value).expect("a view feed")
}

#[test]
fn an_exception_splits_its_stack_and_keys_without_positions() {
    let a = feed(json!({ "view": 1, "errors": [{
        "text": "Uncaught Error: failed at 375 px\n    at f (https://a.test/app.js?v=1:3:9)",
        "url": "https://a.test/app.js?v=1", "line": 3, "column": 9, "time": 1.0 }] }));
    let b = feed(json!({ "view": 2, "errors": [{
        "text": "Uncaught Error: failed at 1280 px\n    at f (https://a.test/app.js?v=2:4:1)",
        "url": "https://a.test/app.js?v=2", "line": 4, "column": 1, "time": 2.0 }] }));
    let (a, b) = (&events(&a)[0], &events(&b)[0]);
    assert_eq!(a.category, Category::JsException);
    assert_eq!(a.text, "Uncaught Error: failed at 375 px");
    assert_eq!(
        a.stack.as_deref(),
        Some("at f (https://a.test/app.js?v=1:3:9)")
    );
    assert_eq!(a.source.as_ref().map(|s| s.line), Some(Some(3)));
    assert_eq!(a.key, b.key);
}

#[test]
fn http_errors_are_404_410_and_5xx_and_failures_without_an_answer_are_not() {
    let view = feed(json!({ "view": 1, "network": [
        { "url": "https://a.test/a.png?_=1731300000000", "method": "GET", "resourceType": "Image", "status": 404, "time": 1.0 },
        { "url": "https://a.test/gone", "status": 410, "time": 1.0 },
        { "url": "https://a.test/api", "method": "POST", "resourceType": "Fetch", "status": 503, "time": 1.0 },
        { "url": "https://a.test/ok", "status": 200, "time": 1.0 },
        { "url": "https://a.test/forbidden", "status": 403, "time": 1.0 },
        { "url": "https://a.test/blocked.js", "failed": "net::ERR_BLOCKED_BY_CLIENT", "time": 1.0 }
    ] }));
    let found = events(&view);
    let categories: Vec<Category> = found.iter().map(|e| e.category).collect();
    assert_eq!(
        categories,
        [
            Category::HttpNotFound,
            Category::HttpNotFound,
            Category::HttpServerError
        ]
    );
    assert_eq!(found[0].key, "http-not-found|404|https://a.test/a.png");
    assert_eq!(
        found[0].text,
        "GET https://a.test/a.png?_=1731300000000 404"
    );
    let request = found[2].request.as_ref().expect("a request");
    assert_eq!(request.method, "POST");
    assert_eq!(request.resource_type.as_deref(), Some("Fetch"));
    assert_eq!(
        found[1].request.as_ref().map(|r| r.method.as_str()),
        Some("GET")
    );
}

#[test]
fn issues_become_csp_mixed_content_and_deprecations() {
    let view = feed(json!({ "view": 1, "issues": [
        { "code": "ContentSecurityPolicyIssue", "kind": "img-src", "detail": "kURLViolation",
          "url": "https://cdn.test/p.png", "sourceUrl": "https://a.test/", "line": 5, "time": 1.0 },
        { "code": "ContentSecurityPolicyIssue", "kind": "script-src-elem",
          "detail": "kInlineViolation", "sourceUrl": "https://a.test/", "reportOnly": true, "time": 1.0 },
        { "code": "MixedContentIssue", "kind": "MixedContentBlocked", "detail": "Script",
          "url": "http://a.test/m.js", "sourceUrl": "https://a.test/", "time": 1.0 },
        { "code": "MixedContentIssue", "kind": "MixedContentAutomaticallyUpgraded", "detail": "Image",
          "url": "http://a.test/m.png", "time": 1.0 },
        { "code": "DeprecationIssue", "kind": "XHRSynchronous", "sourceUrl": "https://a.test/app.js",
          "line": 10, "time": 1.0 },
        { "code": "CookieIssue", "time": 1.0 }
    ], "console": [
        { "level": "error", "source": "security", "text": "Refused to load the image 'https://cdn.test/p.png' because it violates the following Content Security Policy directive: \"img-src 'self'\".", "time": 1.0 }
    ] }));
    let found = events(&view);
    let categories: Vec<Category> = found.iter().map(|e| e.category).collect();
    assert_eq!(
        categories,
        [
            Category::CspViolation,
            Category::CspViolation,
            Category::MixedContentBlocked,
            Category::MixedContentPassive,
            Category::Deprecation
        ],
        "the console's CSP message is not counted beside the issue"
    );
    assert_eq!(found[0].text, "img-src: https://cdn.test/p.png");
    assert_eq!(found[0].url.as_deref(), Some("https://cdn.test/p.png"));
    assert_eq!(
        found[1].key,
        "csp-violation|script-src-elem|inline@https://a.test/"
    );
    assert_eq!(
        found[1].text,
        "script-src-elem: kInlineViolation (report-only)"
    );
    assert_eq!(found[2].text, "Script: http://a.test/m.js");
    assert_eq!(
        found[2].source.as_ref().map(|s| s.url.as_str()),
        Some("https://a.test/")
    );
    assert_eq!(found[4].text, "XHRSynchronous");
    assert_eq!(found[4].detail.as_deref(), Some("XHRSynchronous"));
}

#[test]
fn without_issues_the_console_messages_stand_in() {
    let view = feed(json!({ "view": 1, "console": [
        { "level": "error", "source": "security", "time": 1.0,
          "text": "Mixed Content: The page at 'https://a.test/' was loaded over HTTPS, but requested an insecure script 'http://a.test/m.js'. This request has been blocked; the content must be served over HTTPS." },
        { "level": "warning", "source": "security", "time": 1.0,
          "text": "Mixed Content: The page at 'https://a.test/' was loaded over HTTPS, but requested an insecure element 'http://a.test/i.png'. This request was automatically upgraded to HTTPS." },
        { "level": "error", "source": "security", "time": 1.0, "url": "https://a.test/", "line": 4,
          "text": "Refused to load the image 'https://cdn.test/p.png' because it violates the following Content Security Policy directive: \"img-src 'self'\"." },
        { "level": "warning", "source": "deprecation", "time": 1.0,
          "text": "[Deprecation] Synchronous XMLHttpRequest on the main thread is deprecated." },
        { "level": "error", "source": "network", "time": 1.0,
          "text": "Failed to load resource: the server responded with a status of 404 (Not Found)" },
        { "level": "error", "source": "console-api", "text": "[Deprecation] mine", "time": 1.0 }
    ] }));
    let found = events(&view);
    let categories: Vec<Category> = found.iter().map(|e| e.category).collect();
    assert_eq!(
        categories,
        [
            Category::MixedContentBlocked,
            Category::MixedContentPassive,
            Category::CspViolation,
            Category::Deprecation
        ],
        "a page's own console text is no deprecation"
    );
    assert_eq!(found[0].url.as_deref(), Some("http://a.test/m.js"));
    assert_eq!(found[1].url.as_deref(), Some("http://a.test/i.png"));
    assert_eq!(found[2].url.as_deref(), Some("https://cdn.test/p.png"));
    assert_eq!(found[2].detail.as_deref(), Some("img-src"));
    assert_eq!(found[2].source.as_ref().and_then(|s| s.line), Some(4));
}

#[test]
fn every_category_has_a_rule_a_name_and_a_severity() {
    for category in Category::ALL {
        assert_eq!(Category::parse(category.rule()), Some(category));
        assert!(!category.key_name().contains('-'));
    }
    assert_eq!(Category::Deprecation.severity(), Severity::Info);
    assert_eq!(Category::CspViolation.severity(), Severity::Warning);
    assert_eq!(Category::HttpServerError.severity(), Severity::Error);
}
