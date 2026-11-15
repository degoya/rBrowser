use serde_json::json;

use super::*;
use crate::network_rules::action::{HeaderOp, Rewrite};

fn mock_rule(id: &str) -> Value {
    json!({
        "id": id, "origin": "project",
        "match": { "urlPattern": "*/api/user", "method": "GET", "resourceType": "XHR" },
        "action": { "type": "mock", "status": 500,
                    "headers": [["content-type", "application/json"]],
                    "bodyBase64": "e30=", "delayMs": 800 },
    })
}

#[test]
fn a_mock_rule_reads_with_its_match_and_action() {
    let (document, rejected) = parse(
        &json!({ "rules": [mock_rule("api#user")], "cacheDisabled": true, "clearSiteData": 3 }),
    )
    .expect("reads");
    assert!(rejected.is_empty(), "{rejected:?}");
    assert!(document.cache_disabled);
    assert_eq!(document.clear_site_data, 3);
    let rule = &document.rules[0];
    assert_eq!(
        (rule.id.as_str(), rule.origin),
        ("api#user", Origin::Project)
    );
    assert!(matches!(&rule.matcher.url, UrlPattern::Glob(glob) if glob == "*/api/user"));
    assert_eq!(rule.matcher.method.as_deref(), Some("GET"));
    assert_eq!(rule.matcher.resource_type.as_deref(), Some("XHR"));
    assert_eq!(
        rule.action,
        Action::Mock {
            status: 500,
            headers: vec![("content-type".to_owned(), "application/json".to_owned())],
            body_base64: "e30=".to_owned(),
            delay_ms: 800,
        }
    );
}

#[test]
fn block_and_rewrite_rules_read() {
    let (document, rejected) = parse(&json!({ "rules": [
        { "id": "b", "origin": "settings", "match": { "urlPattern": "*/tracking.js" },
          "action": { "type": "block" } },
        { "id": "r", "origin": "project", "match": { "urlPattern": "^https://a\\.test/", "regex": true },
          "action": { "type": "rewrite", "headers": [["x-test", "1"]] } },
    ] }))
    .expect("reads");
    assert!(rejected.is_empty(), "{rejected:?}");
    assert_eq!(document.rules[0].action, Action::Block);
    assert_eq!(document.rules[0].origin, Origin::Settings);
    assert!(matches!(
        document.rules[1].matcher.url,
        UrlPattern::Regex(_)
    ));
    assert_eq!(
        document.rules[1].action,
        Action::Rewrite(Rewrite {
            headers: vec![HeaderOp::Set("x-test".to_owned(), "1".to_owned())],
            ..Rewrite::default()
        })
    );
}

#[test]
fn the_debugger_parts_of_the_document_read_beside_the_rules() {
    let (document, rejected) = parse(&json!({
        "rules": [mock_rule("m")],
        "breakpoints": [{ "id": "bp-1", "match": { "urlPattern": "*/api/*" } }, { "id": "bad", "match": {} }],
        "breakpointsEnabled": false,
        "settings": { "breakpointTimeoutMs": 10 },
        "asks": [{ "id": "a1", "kind": "abort", "held": "h1" }, { "id": "a2", "kind": "nothing" }],
    }))
    .expect("reads");
    assert_eq!(document.rules.len(), 1);
    assert_eq!(document.breakpoints.len(), 1);
    assert!(!document.breakpoints_enabled);
    assert_eq!(document.settings.map(|s| s.timeout_ms), Some(10));
    assert_eq!(document.asks.len(), 1);
    assert_eq!(
        rejected.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
        ["bad", "a2"]
    );
    let (plain, _) = parse(&json!({})).expect("reads");
    assert!(plain.breakpoints_enabled && plain.settings.is_none() && plain.asks.is_empty());
}

#[test]
fn a_bad_rule_is_rejected_alone_with_its_reason() {
    let mut bad_status = mock_rule("status");
    bad_status["action"]["status"] = json!(99);
    let mut bad_body = mock_rule("body");
    bad_body["action"]["bodyBase64"] = json!("not base64!");
    let mut slow = mock_rule("slow");
    slow["action"]["delayMs"] = json!(MAX_DELAY_MS + 1);
    let mut regex = mock_rule("regex");
    regex["match"] = json!({ "urlPattern": "(", "regex": true });
    let mut header = mock_rule("header");
    header["action"]["headers"] = json!([["x-a", "1\r\nset-cookie: a=b"]]);
    let mut kind = mock_rule("kind");
    kind["match"]["resourceType"] = json!("Anything");
    let mut origin = mock_rule("origin");
    origin["origin"] = json!("plugin");
    let rewrite = json!({ "id": "rewrite", "origin": "project", "match": { "urlPattern": "*" },
                          "action": { "type": "rewrite", "url": "file:///etc/passwd" } });
    let (document, rejected) = parse(&json!({ "rules": [
        mock_rule("ok"), bad_status, bad_body, slow, regex, header, kind, origin, rewrite,
        mock_rule("ok"), { "origin": "project" },
    ] }))
    .expect("reads");
    assert_eq!(document.rules.len(), 1);
    let reasons: Vec<(&str, &str)> = rejected
        .iter()
        .map(|r| (r.id.as_str(), r.reason.as_str()))
        .collect();
    assert_eq!(
        reasons,
        [
            ("status", "bad status"),
            ("body", "bad bodyBase64"),
            ("slow", "bad delayMs"),
            ("regex", "bad regex"),
            ("header", "bad header"),
            ("kind", "bad resourceType"),
            ("origin", "bad origin"),
            ("rewrite", "bad rewrite url"),
            ("ok", "duplicate id"),
            ("#10", "no id"),
        ]
    );
}

#[test]
fn more_than_the_most_rules_are_rejected() {
    let rules: Vec<Value> = (0..=MAX_RULES)
        .map(|n| mock_rule(&format!("r{n}")))
        .collect();
    let (document, rejected) = parse(&json!({ "rules": rules })).expect("reads");
    assert_eq!(document.rules.len(), MAX_RULES);
    assert_eq!(rejected.len(), 1);
    assert_eq!(rejected[0].reason, "too many rules");
}

#[test]
fn a_document_that_is_no_object_applies_nothing() {
    assert!(parse(&json!([])).is_err());
    assert!(parse(&json!({ "rules": "all" })).is_err());
    let (empty, rejected) = parse(&json!({})).expect("an empty document");
    assert!(empty.rules.is_empty() && rejected.is_empty());
    assert!(!empty.cache_disabled);
}
