use rb_plugin_api::RULE_LEVELS_SETTING;
use serde_json::{Value, json};

use super::{check, endpoints, endpoints_check, open};
use crate::fake_host::{FakeHost, view};

/// Mobile loads the cart twice and the teaser three times, Desktop once each.
fn capture() -> Value {
    let entry = |id: &str, url: &str, time: f64, ms: f64| {
        json!({ "id": id, "url": url, "method": "GET", "resourceType": "Fetch", "status": 200,
                "mimeType": "application/json", "size": 300.0, "time": time, "durationMs": ms,
                "response": { "ttfbMs": ms / 2.0, "encoding": "br", "cacheControl": "no-store",
                              "expires": false, "etag": false, "lastModified": false } })
    };
    json!({ "$console": { "views": [
        { "view": 1, "network": [
            entry("1.1", "https://a.test/api/cart?token=secret", 1.0, 100.0),
            entry("1.2", "https://a.test/api/cart?token=secret", 2.0, 120.0),
            entry("1.3", "https://a.test/api/teaser/1", 3.0, 40.0),
            entry("1.4", "https://a.test/api/teaser/2", 4.0, 40.0),
            entry("1.5", "https://a.test/api/teaser/3", 5.0, 40.0),
            { "id": "1.6", "url": "https://a.test/app.js", "method": "GET", "resourceType": "Script", "time": 6.0 },
        ] },
        { "view": 2, "network": [
            entry("2.1", "https://a.test/api/cart?token=secret", 1.0, 3000.0),
            entry("2.2", "https://a.test/api/teaser/1", 2.0, 30.0),
        ] },
    ] } })
}

fn host() -> FakeHost {
    FakeHost::with_views(vec![view(1, "Mobile", 390), view(2, "Desktop", 1440)])
}

#[test]
fn the_endpoints_answer_groups_the_api_calls_slowest_first_masked() {
    let answer = endpoints(&mut host(), &capture()).expect("an answer");
    let keys: Vec<&str> = answer["endpoints"]
        .as_array()
        .expect("endpoints")
        .iter()
        .filter_map(|endpoint| endpoint["key"].as_str())
        .collect();
    assert_eq!(
        keys,
        [
            "GET https://a.test/api/cart",
            "GET https://a.test/api/teaser/{id}"
        ]
    );
    assert_eq!(answer["total"], 2);
    assert_eq!(answer["captured"], true);
    assert_eq!(answer["views"][1]["label"], "Desktop");
    let cart = &answer["endpoints"][0];
    assert_eq!(cart["calls"], 3);
    assert_eq!(cart["durationMs"]["p95"], 3000.0);
    let issues: Vec<&str> = cart["issues"]
        .as_array()
        .expect("issues")
        .iter()
        .filter_map(Value::as_str)
        .collect();
    assert_eq!(issues, ["endpoint.duplicate", "endpoint.slow"]);
    let rules: Vec<&str> = answer["findings"]
        .as_array()
        .expect("findings")
        .iter()
        .filter_map(|finding| finding["rule"].as_str())
        .collect();
    assert!(rules.contains(&"endpoint.breakpoint-calls"), "{rules:?}");
    assert_eq!(
        answer["findings"][0]["endpoint"],
        "GET https://a.test/api/cart"
    );
    assert!(!answer.to_string().contains("secret"));
    assert!(answer.to_string().len() < 64 * 1024);
}

#[test]
fn the_arguments_choose_view_types_sort_and_search() {
    let mut host = host();
    let only = endpoints(&mut host, &{
        let mut args = capture();
        args["target"] = json!(2);
        args["sort"] = json!("calls");
        args
    })
    .expect("an answer");
    assert_eq!(only["endpoints"][0]["calls"], 1);
    let all = endpoints(&mut host, &{
        let mut args = capture();
        args["types"] = json!([]);
        args["search"] = json!("APP");
        args
    })
    .expect("an answer");
    assert_eq!(all["endpoints"][0]["key"], "GET https://a.test/app.js");
    for bad in [
        json!({ "sort": "weird" }),
        json!({ "limit": 0 }),
        json!({ "types": "Fetch" }),
        json!({ "types": ["Nonsense"] }),
        json!({ "search": 3 }),
    ] {
        assert!(endpoints(&mut host, &bad).is_err(), "{bad}");
    }
}

#[test]
fn the_check_answers_the_rules_asked_for_in_the_view_asked_for() {
    let mut host = host();
    let found = check(
        &mut host,
        &capture(),
        &["endpoint.duplicate".to_owned()],
        Some(1),
    );
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].view, Some(1));
    assert!(
        check(
            &mut host,
            &capture(),
            &["endpoint.duplicate".to_owned()],
            Some(2)
        )
        .is_empty()
    );
}

#[test]
fn the_person_s_levels_switch_a_rule_off() {
    let mut host = host();
    host.set(
        RULE_LEVELS_SETTING,
        r#"{ "endpoint.slow": "off", "endpoint.duplicate": "error" }"#,
    );
    let found = check(&mut host, &capture(), &[], None);
    assert!(found.iter().all(|finding| finding.rule != "endpoint.slow"));
    let duplicate = found
        .iter()
        .find(|finding| finding.rule == "endpoint.duplicate")
        .expect("a duplicate");
    assert_eq!(duplicate.severity.as_str(), "error");
}

#[test]
fn the_command_reports_the_findings() {
    let mut host = host();
    let answer = endpoints_check(&mut host, &capture()).expect("an answer");
    assert!(
        answer["findings"].as_u64().is_some_and(|n| n >= 3),
        "{answer}"
    );
    assert_eq!(answer["reported"], true);
    assert!(!host.findings.is_empty());
}

#[test]
fn the_command_opens_the_endpoints_tab() {
    let answer = open();
    assert_eq!(answer["$open"]["panel"], "network");
    assert_eq!(answer["$post"][0]["message"]["tab"], "endpoints");
}

#[test]
fn without_a_capture_the_answer_says_so() {
    let answer = endpoints(&mut host(), &json!({})).expect("an answer");
    assert_eq!(answer["captured"], false);
    assert_eq!(answer["endpoints"], json!([]));
}
