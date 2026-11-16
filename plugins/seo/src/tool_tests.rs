use rb_plugin_api::test_support::MemoryHost;
use rb_plugin_api::{Capability, HttpResponse};

use super::*;

fn host() -> MemoryHost {
    let mut host = MemoryHost::new()
        .grant(Capability::Http)
        .grant(Capability::Views)
        .grant(Capability::Evaluate)
        .grant(Capability::OutWrite)
        .grant(Capability::Findings);
    host.now = 1_762_869_845;
    host.responses.insert(
        "https://shop.test/".to_owned(),
        HttpResponse {
            status: 200,
            headers: vec![("content-type".to_owned(), "text/html".to_owned())],
            body: b"<html><body><img src=a.jpg></body></html>".to_vec(),
        },
    );
    host
}

fn answer(host: &mut MemoryHost, args: &str) -> Value {
    serde_json::from_str(&check(host, args).expect("answer")).expect("json")
}

#[test]
fn the_tool_writes_the_report_reports_the_findings_and_answers_in_short() {
    let mut host = host();
    let answer = answer(
        &mut host,
        r#"{"url": "https://shop.test/", "scope": "page"}"#,
    );
    assert_eq!(answer["run"], "20251111-140405");
    assert_eq!(answer["pages"], 1);
    assert_eq!(answer["images"]["total"], 1);
    assert_eq!(answer["images"]["ok"], 0);
    assert_eq!(
        answer["report"]["json"],
        ".rbrowser/out/seo/20251111-140405/report.json"
    );
    assert!(host.out.contains_key("20251111-140405/report.html"));
    assert!(!host.findings.is_empty(), "the findings panel got them");
    let first = &answer["findings"][0];
    assert_eq!(first["severity"], "error");
    assert!(
        first["text"]
            .as_str()
            .is_some_and(|text| !text.starts_with("plugin."))
    );
    assert_eq!(answer["truncated"], false);
}

#[test]
fn a_view_target_is_compared_and_names_the_run() {
    let mut host = host();
    host.evaluate_answer = serde_json::json!({
        "url": "https://shop.test/",
        "html": "<html><body><img src=a.jpg alt=\"A red bike\"></body></html>"
    })
    .to_string();
    let answer = answer(&mut host, r#"{"url": "https://shop.test/", "target": 2}"#);
    assert_eq!(answer["run"], "20251111-140405-v2");
    assert_eq!(answer["rendered"], true);
    assert_eq!(host.evaluated[0].0, 2);
}

#[test]
fn without_out_write_the_answer_still_comes() {
    let mut host = MemoryHost::new().grant(Capability::Http);
    host.responses.insert(
        "https://shop.test/".to_owned(),
        HttpResponse {
            status: 200,
            ..HttpResponse::default()
        },
    );
    let answer = answer(&mut host, r#"{"url": "https://shop.test/"}"#);
    assert!(answer["report"].is_null());
}

#[test]
fn malformed_arguments_are_refused_with_the_field() {
    let mut host = host();
    for (args, reason) in [
        ("not json", "json"),
        (r#"{"scope": "site"}"#, "scope"),
        (r#"{"maxPages": 0}"#, "maxPages"),
        (r#"{"target": true}"#, "target"),
        (r#"{"other": 1}"#, "fields"),
    ] {
        let failure = check(&mut host, args).expect_err(args);
        assert_eq!(failure.code(), Some("plugin.seo.errors.bad_args"), "{args}");
        assert_eq!(failure.param("reason"), Some(reason), "{args}");
    }
}

#[test]
fn the_hreflang_scope_answers_its_set_and_parity_its_views() {
    let mut host = host();
    let answer = answer(
        &mut host,
        r#"{"url": "https://shop.test/", "scope": "hreflang", "parity": true}"#,
    );
    assert_eq!(answer["scope"], "hreflang");
    assert_eq!(answer["hreflang"]["pages"], 1);
    assert_eq!(answer["hreflang"]["missing"], 0);
    assert!(answer["parity"].is_null(), "no two views show the page");
    let rules: Vec<&str> = answer["findings"]
        .as_array()
        .expect("findings")
        .iter()
        .filter_map(|finding| finding["rule"].as_str())
        .collect();
    assert_eq!(rules, ["hreflang.none"]);
    let failure = check(&mut host, r#"{"parity": "yes"}"#).expect_err("not a boolean");
    assert_eq!(failure.code(), Some("plugin.seo.errors.bad_args"));
}

#[test]
fn a_command_s_fixed_argument_goes_under_what_the_caller_sent() {
    assert_eq!(
        with("", "scope", "hreflang".into()),
        r#"{"scope":"hreflang"}"#
    );
    assert_eq!(
        with(
            r#"{"scope": "page", "url": "u"}"#,
            "scope",
            "hreflang".into()
        ),
        r#"{"scope":"page","url":"u"}"#
    );
    assert_eq!(with("[1]", "parity", true.into()), "[1]");
    assert_eq!(with("{bad", "parity", true.into()), "{bad");
}
