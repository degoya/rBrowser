use serde_json::json;

use super::*;
use crate::network_rules::rule::parse;

fn rules(raw: Value) -> Vec<Rule> {
    let (document, rejected) = parse(&json!({ "rules": raw })).expect("reads");
    assert!(rejected.is_empty(), "{rejected:?}");
    document.rules
}

fn paused(url: &str, method: &str, kind: &str) -> Paused {
    Paused::of(&json!({
        "requestId": "interception-1",
        "request": { "url": url, "method": method, "headers": { "Accept": "*/*", "X-Old": "a" } },
        "resourceType": kind,
    }))
    .expect("a paused request")
}

fn response_paused(url: &str) -> Paused {
    Paused::of(&json!({
        "requestId": "interception-2",
        "request": { "url": url, "method": "GET", "headers": {} },
        "resourceType": "Document",
        "responseStatusCode": 200,
        "responseHeaders": [{ "name": "content-type", "value": "text/html" },
                            { "name": "content-security-policy", "value": "default-src 'self'" }],
    }))
    .expect("a paused response")
}

/// The one call of `outcome`.
fn call(outcome: Outcome) -> Decision {
    match outcome {
        Outcome::Call(decision) => decision,
        other => panic!("one call, not {other:?}"),
    }
}

#[test]
fn rules_match_by_glob_regex_method_and_type() {
    let rules = rules(json!([
        { "id": "glob", "origin": "project", "match": { "urlPattern": "*/api/user", "method": "GET" },
          "action": { "type": "block" } },
        { "id": "regex", "origin": "project",
          "match": { "urlPattern": "/img/[a-z]+\\.png$", "regex": true, "resourceType": "Image" },
          "action": { "type": "block" } },
    ]));
    let hit =
        |url, method, kind| first_match(&rules, &paused(url, method, kind)).map(|r| r.id.clone());
    assert_eq!(
        hit("http://a.test/api/user", "GET", "XHR").as_deref(),
        Some("glob")
    );
    assert_eq!(hit("http://a.test/api/user", "POST", "XHR"), None, "method");
    assert_eq!(
        hit("http://a.test/img/cat.png", "GET", "Image").as_deref(),
        Some("regex")
    );
    assert_eq!(
        hit("http://a.test/img/cat.png", "GET", "Fetch"),
        None,
        "resource type"
    );
    assert_eq!(
        hit("http://a.test/img/Cat1.png", "GET", "Image"),
        None,
        "regex"
    );
}

#[test]
fn the_first_matching_rule_wins() {
    let rules = rules(json!([
        { "id": "first", "origin": "project", "match": { "urlPattern": "*" }, "action": { "type": "block" } },
        { "id": "second", "origin": "project", "match": { "urlPattern": "*" }, "action": { "type": "block" } },
    ]));
    let decision = call(decide(&rules, &paused("http://a.test/", "GET", "Document")));
    assert_eq!(decision.rule.as_deref(), Some("first"));
}

#[test]
fn a_mock_fulfils_after_its_delay() {
    let rules = rules(
        json!([{ "id": "m", "origin": "project", "match": { "urlPattern": "*/api/user" },
        "action": { "type": "mock", "status": 500, "headers": [["content-type", "application/json"]],
                    "bodyBase64": "e30=", "delayMs": 800 } }]),
    );
    let decision = call(decide(
        &rules,
        &paused("http://a.test/api/user", "GET", "XHR"),
    ));
    assert_eq!(decision.method, "Fetch.fulfillRequest");
    assert_eq!(decision.delay, Duration::from_millis(800));
    assert_eq!(
        decision.params,
        json!({ "requestId": "interception-1", "responseCode": 500,
                "responseHeaders": [{ "name": "content-type", "value": "application/json" }],
                "body": "e30=" })
    );
}

#[test]
fn a_block_fails_the_request_as_blocked_by_client() {
    let rules = rules(
        json!([{ "id": "b", "origin": "settings", "match": { "urlPattern": "*/tracking.js" },
        "action": { "type": "block" } }]),
    );
    let decision = call(decide(
        &rules,
        &paused("http://a.test/tracking.js", "GET", "Script"),
    ));
    assert_eq!(decision.method, "Fetch.failRequest");
    assert_eq!(
        decision.params,
        json!({ "requestId": "interception-1", "errorReason": "BlockedByClient" })
    );
}

#[test]
fn a_rewrite_replaces_a_header_case_blind_and_keeps_the_others() {
    let rules = rules(
        json!([{ "id": "r", "origin": "project", "match": { "urlPattern": "*" },
        "action": { "type": "rewrite", "url": "http://b.test/new",
                    "headers": [["x-old", "b"], ["X-New", "c"]] } }]),
    );
    let decision = call(decide(&rules, &paused("http://a.test/", "GET", "Fetch")));
    assert_eq!(decision.method, "Fetch.continueRequest");
    assert_eq!(decision.params["url"], "http://b.test/new");
    assert_eq!(
        decision.params["headers"],
        json!([{ "name": "Accept", "value": "*/*" }, { "name": "x-old", "value": "b" },
               { "name": "X-New", "value": "c" }])
    );
}

#[test]
fn no_matching_rule_continues_unchanged() {
    let decision = call(decide(&[], &paused("http://a.test/", "GET", "Document")));
    assert_eq!(decision.rule, None);
    assert_eq!(decision.method, "Fetch.continueRequest");
    assert_eq!(decision.params, json!({ "requestId": "interception-1" }));
    assert_eq!(decision.delay, Duration::ZERO);
    let response = call(decide(&[], &response_paused("http://a.test/")));
    assert_eq!(response.method, "Fetch.continueResponse");
    assert_eq!(response.params, json!({ "requestId": "interception-2" }));
}

#[test]
fn an_extended_rewrite_changes_query_and_headers_at_the_request_stage() {
    let rules = rules(
        json!([{ "id": "r", "origin": "project", "match": { "urlPattern": "*" },
        "action": { "type": "rewrite",
                    "query": [{ "op": "set", "name": "debug", "value": "1" }],
                    "requestHeaders": [{ "op": "remove", "name": "x-old" }],
                    "responseHeaders": [{ "op": "remove", "name": "content-security-policy" }] } }]),
    );
    let decision = call(decide(
        &rules,
        &paused("http://a.test/p?a=1", "GET", "Fetch"),
    ));
    assert_eq!(decision.method, "Fetch.continueRequest");
    assert_eq!(decision.params["url"], "http://a.test/p?a=1&debug=1");
    assert_eq!(
        decision.params["headers"],
        json!([{ "name": "Accept", "value": "*/*" }])
    );
    let Outcome::Response {
        rule,
        request_id,
        status,
        response_headers,
        headers,
        body,
    } = decide(&rules, &response_paused("http://a.test/p"))
    else {
        panic!("a response rewrite");
    };
    assert_eq!(
        (rule.as_str(), request_id.as_str(), status),
        ("r", "interception-2", 200)
    );
    assert_eq!(response_headers.len(), 2);
    assert_eq!(
        headers,
        vec![HeaderOp::Remove("content-security-policy".to_owned())]
    );
    assert!(body.is_none());
}

#[test]
fn a_response_only_rewrite_lets_the_request_stage_pass_to_the_next_rule() {
    let rules = rules(json!([
        { "id": "late", "origin": "project", "match": { "urlPattern": "*" },
          "action": { "type": "rewrite", "body": { "regex": "a", "replacement": "b" } } },
        { "id": "block", "origin": "project", "match": { "urlPattern": "*" }, "action": { "type": "block" } },
    ]));
    assert_eq!(
        call(decide(&rules, &paused("http://a.test/", "GET", "Document")))
            .rule
            .as_deref(),
        Some("block")
    );
    assert!(matches!(
        decide(&rules, &response_paused("http://a.test/")),
        Outcome::Response { rule, body: Some(_), .. } if rule == "late"
    ));
}

#[test]
fn map_remote_swaps_the_url_and_sets_host_and_origin() {
    let rules = rules(
        json!([{ "id": "m", "origin": "project", "match": { "urlPattern": "https://api.example.com/*" },
        "action": { "type": "mapRemote", "from": { "host": "api.example.com" },
                    "to": { "scheme": "http", "host": "localhost", "port": 3001 } } }]),
    );
    let mut request = paused("https://api.example.com/v1/users", "POST", "Fetch");
    request
        .headers
        .push(("Origin".to_owned(), "https://api.example.com".to_owned()));
    let decision = call(decide(&rules, &request));
    assert_eq!(decision.method, "Fetch.continueRequest");
    assert_eq!(decision.params["url"], "http://localhost:3001/v1/users");
    let headers = decision.params["headers"]
        .as_array()
        .expect("headers")
        .clone();
    assert!(headers.contains(&json!({ "name": "Host", "value": "localhost:3001" })));
    assert!(headers.contains(&json!({ "name": "Origin", "value": "http://localhost:3001" })));
    assert!(headers.contains(&json!({ "name": "Accept", "value": "*/*" })));
    let other = call(decide(
        &rules,
        &paused("https://api.example.com:8443/v1", "GET", "Fetch"),
    ));
    assert_eq!(other.rule, None, "another port: the rule does not cover it");
}

#[test]
fn map_local_without_a_resolved_folder_does_not_apply_and_with_one_serves_the_folder() {
    let (document, _) = parse(&json!({ "rules": [{ "id": "l", "origin": "project",
        "match": { "urlPattern": "*/config.json" },
        "action": { "type": "mapLocal", "folder": "fixtures" } }] }))
    .expect("reads");
    let mut rules = document.rules;
    assert_eq!(
        call(decide(
            &rules,
            &paused("http://a.test/config.json", "GET", "Fetch")
        ))
        .rule,
        None
    );
    if let Action::MapLocal(local) = &mut rules[0].action {
        local.resolved = Some(std::path::PathBuf::from("/tmp/fixtures"));
    }
    assert!(matches!(
        decide(&rules, &paused("http://a.test/config.json", "GET", "Fetch")),
        Outcome::Local { rule, root, file: None, mime: None, .. }
            if rule == "l" && root == std::path::Path::new("/tmp/fixtures")
    ));
}

#[test]
fn a_paused_response_reads_its_status_headers_and_stage() {
    let response = response_paused("http://a.test/");
    assert_eq!(response.stage, Stage::Response);
    assert_eq!(response.response_status, Some(200));
    assert_eq!(
        response.response_headers[0],
        ("content-type".to_owned(), "text/html".to_owned())
    );
    let request = Paused::of(
        &json!({ "requestId": "r", "request": { "url": "http://a.test/", "method": "POST",
        "headers": {}, "postData": "a=1", "hasPostData": true } }),
    )
    .expect("reads");
    assert_eq!(request.stage, Stage::Request);
    assert_eq!(request.post_data.as_deref(), Some(b"a=1".as_slice()));
}

#[test]
fn breakpoint_patterns_follow_the_enabled_breakpoints_and_their_stages() {
    let (breakpoints, _) = crate::network_rules::document::parse_breakpoints(&json!([
        { "id": "a", "match": { "urlPattern": "*/api/*" }, "stage": "response" },
        { "id": "b", "match": { "urlPattern": "*/api/*" }, "stage": "response" },
        { "id": "c", "match": { "urlPattern": "*/x" }, "enabled": false },
    ]));
    assert_eq!(
        breakpoint_patterns(&breakpoints),
        vec![json!({ "urlPattern": "*/api/*", "requestStage": "Response" })]
    );
}

#[test]
fn fetch_patterns_follow_the_rules() {
    let rules = rules(json!([
        { "id": "a", "origin": "project", "match": { "urlPattern": "*/api/*", "resourceType": "XHR" },
          "action": { "type": "block" } },
        { "id": "b", "origin": "project", "match": { "urlPattern": "*/api/*", "resourceType": "XHR" },
          "action": { "type": "block" } },
        { "id": "c", "origin": "project", "match": { "urlPattern": "x", "regex": true },
          "action": { "type": "block" } },
        { "id": "d", "origin": "project", "match": { "urlPattern": "*/page" },
          "action": { "type": "rewrite", "responseHeaders": [{ "op": "remove", "name": "x" }],
                      "query": [{ "op": "remove", "name": "y" }] } },
    ]));
    assert_eq!(
        patterns(&rules),
        vec![
            json!({ "urlPattern": "*/api/*", "resourceType": "XHR", "requestStage": "Request" }),
            json!({ "urlPattern": "*", "requestStage": "Request" }),
            json!({ "urlPattern": "*/page", "requestStage": "Request" }),
            json!({ "urlPattern": "*/page", "requestStage": "Response" }),
        ]
    );
    assert!(patterns(&[]).is_empty());
}

#[test]
fn a_paused_request_without_an_id_is_none() {
    assert!(Paused::of(&json!({ "request": { "url": "http://a.test/" } })).is_none());
}

#[test]
fn a_failed_answer_falls_back_to_continue_then_fail_and_a_block_stays_blocked() {
    let id = json!("R1");
    let plain = ("Fetch.continueRequest", json!({ "requestId": id }));
    let failed = (
        "Fetch.failRequest",
        json!({ "requestId": id, "errorReason": "Failed" }),
    );
    let decision = |method: &'static str, params: Value| Decision {
        rule: None,
        delay: Duration::ZERO,
        method,
        params,
    };
    let mock = decision(
        "Fetch.fulfillRequest",
        json!({ "requestId": id, "responseCode": 500 }),
    );
    assert_eq!(fallbacks(&mock), [plain.clone(), failed.clone()]);
    let rewrite = decision(
        "Fetch.continueRequest",
        json!({ "requestId": id, "url": "http://x/" }),
    );
    assert_eq!(fallbacks(&rewrite), [plain.clone(), failed.clone()]);
    let unchanged = decision("Fetch.continueRequest", json!({ "requestId": id }));
    assert_eq!(fallbacks(&unchanged), std::slice::from_ref(&failed));
    let response = decision(
        "Fetch.continueResponse",
        json!({ "requestId": id, "responseCode": 500 }),
    );
    assert_eq!(
        fallbacks(&response),
        [("Fetch.continueResponse", plain.1.clone()), failed]
    );
    let block = decision(
        "Fetch.failRequest",
        json!({ "requestId": id, "errorReason": "BlockedByClient" }),
    );
    assert!(
        fallbacks(&block).is_empty(),
        "never let a blocked request through"
    );
}
