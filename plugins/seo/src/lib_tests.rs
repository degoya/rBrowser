use rb_plugin_api::test_support::MemoryHost;
use rb_plugin_api::{Capability, HttpResponse};

use super::*;

fn host() -> MemoryHost {
    let mut host = MemoryHost::new()
        .grant(Capability::Http)
        .grant(Capability::Views)
        .grant(Capability::Evaluate);
    host.responses.insert(
        "https://shop.test/".to_owned(),
        HttpResponse {
            status: 200,
            headers: vec![("content-type".to_owned(), "text/html".to_owned())],
            body: b"<html><body><h1>A</h1><h1>B</h1></body></html>".to_vec(),
        },
    );
    host
}

#[test]
fn the_check_runs_the_rules_it_is_asked_for() {
    let mut host = host();
    let ctx = CheckContext {
        view: None,
        url: Some("https://shop.test/".to_owned()),
        rules: vec!["headings.h1_multiple".to_owned()],
        args: "{}".to_owned(),
    };
    let findings = Seo.run(&mut host, &ctx).expect("run");
    let rules: Vec<&str> = findings.iter().map(|f| f.rule.as_str()).collect();
    assert_eq!(rules, ["headings.h1_multiple"]);
    assert!(host.out.is_empty(), "a check run writes no report");
    assert_eq!(Seo.describe().len(), rules::RULES.len());
}

#[test]
fn the_command_and_the_tool_are_one_and_others_are_unknown() {
    let mut host = host();
    let answer = Seo
        .run_command(&mut host, "check", r#"{"url": "https://shop.test/"}"#)
        .expect("command");
    assert!(answer.contains("\"pages\":1"));
    let failure = Seo.call_tool(&mut host, "nope", "{}").expect_err("unknown");
    assert_eq!(failure.code(), Some(codes::UNKNOWN_CONTRIBUTION));
    let failure = Seo
        .run_command(&mut host, "@page", "{}")
        .expect_err("no page script");
    assert_eq!(failure.code(), Some(codes::UNKNOWN_CONTRIBUTION));
}

#[test]
fn the_hreflang_and_parity_commands_run_the_check_with_their_argument() {
    let mut host = host();
    let answer = Seo
        .run_command(&mut host, "hreflang", r#"{"url": "https://shop.test/"}"#)
        .expect("command");
    assert!(answer.contains("\"scope\":\"hreflang\""), "{answer}");
    let answer = Seo
        .run_command(&mut host, "parity", r#"{"url": "https://shop.test/"}"#)
        .expect("command");
    assert!(answer.contains("\"scope\":\"page\""), "{answer}");
    let ctx = CheckContext {
        view: None,
        url: Some("https://shop.test/".to_owned()),
        rules: Vec::new(),
        args: r#"{"scope": "hreflang"}"#.to_owned(),
    };
    let findings = Seo.run(&mut host, &ctx).expect("run");
    assert!(findings.iter().all(|f| f.rule.starts_with("hreflang.")));
}
