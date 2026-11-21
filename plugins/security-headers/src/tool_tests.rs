use rb_plugin_api::RULE_LEVELS_SETTING;
use serde_json::json;

use super::*;
use crate::testing::{URL, good, host, serve, view, with_environments};

fn args(value: Value) -> Map<String, Value> {
    value.as_object().cloned().expect("object")
}

fn without(name: &str) -> Vec<(&'static str, &'static str)> {
    good().into_iter().filter(|(own, _)| *own != name).collect()
}

#[test]
fn the_check_answers_the_headers_cookies_and_findings_of_the_active_view() {
    let mut host = host();
    let mut headers = without("strict-transport-security");
    headers.push(("set-cookie", "sid=NEVER-SHOWN; Path=/"));
    serve(&mut host, URL, &headers);
    view(&mut host, 1, URL);
    with_environments(&mut host, &[("prod", "https://example.test")]);
    let answer = check(&mut host, &Map::new()).expect("answer");
    assert_eq!(answer["environment"], "prod");
    assert_eq!(answer["status"], 200);
    assert_eq!(answer["headers"]["x-frame-options"], "DENY");
    assert_eq!(answer["headers"]["strict-transport-security"], Value::Null);
    assert_eq!(
        answer["cookies"][0],
        json!({ "name": "sid", "attributes": "" })
    );
    let rules: Vec<&str> = answer["findings"]
        .as_array()
        .expect("findings")
        .iter()
        .filter_map(|finding| finding["rule"].as_str())
        .collect();
    assert_eq!(rules, ["hsts.missing", "cookie.flags"]);
    assert_eq!(answer["summary"]["warning"], 2);
    assert!(!answer.to_string().contains("NEVER-SHOWN"));
}

#[test]
fn the_check_takes_a_url_and_refuses_anything_else() {
    let mut host = host();
    serve(&mut host, "https://example.test/a", &good());
    let answer =
        check(&mut host, &args(json!({ "url": "https://example.test/a" }))).expect("answer");
    assert_eq!(answer["url"], "https://example.test/a");
    for url in [
        json!("file:///etc/passwd"),
        json!(3),
        json!("https://u@x.test/"),
        json!(format!("https://x.test/{}", "a".repeat(3000))),
    ] {
        let failure = check(&mut host, &args(json!({ "url": url }))).expect_err("refused");
        assert_eq!(failure.code(), Some(codes::INVALID));
    }
    assert_eq!(host.fetched.len(), 1);
}

#[test]
fn without_a_web_page_the_check_says_so() {
    let mut host = host();
    let failure = check(&mut host, &Map::new()).expect_err("no view");
    assert_eq!(
        failure.code(),
        Some("plugin.security-headers.errors.no_page")
    );
    view(&mut host, 1, "about:blank");
    let failure = check(&mut host, &Map::new()).expect_err("blank");
    assert_eq!(
        failure.code(),
        Some("plugin.security-headers.errors.no_page")
    );
}

#[test]
fn the_persons_levels_apply_to_the_answer() {
    let mut host = host();
    serve(&mut host, URL, &without("strict-transport-security"));
    view(&mut host, 1, URL);
    host.settings.insert(
        RULE_LEVELS_SETTING.to_owned(),
        r#"{ "hsts.missing": "off" }"#.to_owned(),
    );
    let answer = check(&mut host, &Map::new()).expect("answer");
    assert_eq!(answer["findings"], json!([]));
}

#[test]
fn the_command_reports_the_findings() {
    let mut host = host();
    serve(&mut host, URL, &without("x-content-type-options"));
    view(&mut host, 1, URL);
    view(&mut host, 2, URL);
    let answer = check_command(&mut host, &args(json!({ "target": 2 }))).expect("answer");
    assert_eq!(answer["findings"], 1);
    assert_eq!(answer["reported"], true);
    assert_eq!(host.findings.len(), 1);
    assert_eq!(host.findings[0].rule, "nosniff.missing");
    assert_eq!(host.findings[0].view, None);
}

#[test]
fn the_comparison_reads_each_environment_at_the_active_path() {
    let mut host = host();
    with_environments(
        &mut host,
        &[
            ("local", "http://localhost:3000"),
            ("staging", "https://staging.example.test"),
            ("prod", "https://example.test"),
        ],
    );
    view(&mut host, 1, "https://example.test/shop?x=1#top");
    let mut staging = good();
    staging.retain(|(name, _)| *name != "content-security-policy");
    staging.push(("content-security-policy", "script-src 'self' 'unsafe-inline'; object-src 'none'; base-uri 'self'; frame-ancestors 'none'"));
    serve(&mut host, "https://staging.example.test/shop?x=1", &staging);
    serve(&mut host, "https://example.test/shop?x=1", &good());
    let table = compare(
        &mut host,
        &args(json!({ "environments": ["staging", "prod"] })),
    )
    .expect("table");
    assert_eq!(table["path"], "/shop?x=1");
    let differing: Vec<&str> = table["rows"]
        .as_array()
        .expect("rows")
        .iter()
        .filter(|row| row["differs"] == json!(true))
        .filter_map(|row| row["header"].as_str())
        .collect();
    assert_eq!(differing, ["content-security-policy"]);
    assert_eq!(
        table["environments"][0]["findings"][0]["rule"],
        "csp.unsafe"
    );
    assert_eq!(table["environments"][1]["findings"], json!([]));
    assert!(host.cdp_sent.is_empty());
    let all = compare(&mut host, &Map::new()).expect("all");
    assert_eq!(all["environments"].as_array().map(Vec::len), Some(3));
    assert!(all["environments"][0]["error"]["code"].is_string());
}

#[test]
fn the_comparison_needs_two_known_environments() {
    let mut host = host();
    with_environments(&mut host, &[("prod", "https://example.test")]);
    let failure = compare(&mut host, &Map::new()).expect_err("one");
    assert_eq!(
        failure.code(),
        Some("plugin.security-headers.errors.environments")
    );
    with_environments(
        &mut host,
        &[
            ("prod", "https://example.test"),
            ("staging", "https://staging.example.test"),
        ],
    );
    let failure = compare(
        &mut host,
        &args(json!({ "environments": ["prod", "nope"] })),
    )
    .expect_err("unknown");
    assert_eq!(
        failure.code(),
        Some("plugin.security-headers.errors.unknown_environment")
    );
    assert_eq!(failure.param("environment"), Some("nope"));
    let failure = compare(&mut host, &args(json!({ "environments": "prod" }))).expect_err("bad");
    assert_eq!(failure.code(), Some(codes::INVALID));
    let failure = compare(
        &mut host,
        &args(json!({ "environments": ["prod", "prod"] })),
    )
    .expect_err("twice");
    assert_eq!(
        failure.code(),
        Some("plugin.security-headers.errors.environments")
    );
}
