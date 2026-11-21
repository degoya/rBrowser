use rb_plugin_api::codes;
use serde_json::json;

use super::*;
use crate::testing::{PAGE, URL, host, view};

fn args(value: Value) -> Map<String, Value> {
    match value {
        Value::Object(map) => map,
        _ => Map::new(),
    }
}

#[test]
fn arguments_are_an_object_or_nothing() {
    assert!(parse("").expect("empty").is_empty());
    assert!(parse("null").expect("null").is_empty());
    assert_eq!(
        parse("[1]").expect_err("array").code(),
        Some(codes::INVALID)
    );
}

#[test]
fn inspect_answers_the_tree_the_blocks_and_the_findings_at_their_nodes() {
    let mut host = host(PAGE);
    view(&mut host, 1, URL);
    let answer = inspect(&mut host, &args(json!({ "target": 1 }))).expect("answer");
    assert_eq!(answer["view"]["id"], 1);
    assert_eq!(answer["vocabulary"], "30.1");
    assert_eq!(answer["blocks"].as_array().map(Vec::len), Some(2));
    assert_eq!(answer["items"].as_array().map(Vec::len), Some(3));
    assert_eq!(answer["items"][1]["types"], json!(["Article"]));
    assert_eq!(answer["items"][1]["source"]["syntax"], "json-ld");
    let finding = &answer["findings"][0];
    assert_eq!(finding["rule"], "property.unknown");
    assert_eq!(finding["severity"], "error");
    assert_eq!(finding["node"], "n1");
    assert_eq!(finding["property"], "headlinee");
    assert_eq!(finding["params"]["property"], "headlinee");
    assert_eq!(
        answer["summary"],
        json!({ "error": 1, "warning": 0, "info": 0 })
    );
    assert_eq!(answer["cut"], false);
    assert_eq!(answer["views"][0]["label"], "View 1");
}

#[test]
fn the_minimum_severity_hides_the_milder_findings() {
    let mut host = host(PAGE);
    view(&mut host, 1, URL);
    host.settings
        .insert("minSeverity".to_owned(), "\"error\"".to_owned());
    host.settings
        .insert("richResults".to_owned(), "false".to_owned());
    let answer = inspect(&mut host, &Map::new()).expect("answer");
    assert_eq!(answer["minSeverity"], "error");
    assert_eq!(answer["richResults"], false);
    assert_eq!(answer["findings"].as_array().map(Vec::len), Some(1));
}

#[test]
fn a_budget_drops_the_raw_texts_then_the_last_items_but_keeps_the_findings() {
    let block = r#"<script type="application/ld+json">{"@context": "https://schema.org", "@type": "Thing", "name": "A thing with a name long enough to count", "description": "And a description of some length as well, to fill the answer."}</script>"#;
    let big = PAGE.replace("</head>", &format!("{}</head>", block.repeat(40)));
    let mut host = host(&big);
    view(&mut host, 1, URL);
    let full = inspect(&mut host, &Map::new()).expect("answer");
    let fits = fitted(full.clone(), Some(4_000_000));
    assert_eq!(fits, full);
    let lean = fitted(full.clone(), Some(16_384));
    assert_eq!(lean["cut"], true);
    assert!(lean["blocks"][0]["text"].is_null());
    assert!(lean.to_string().len() <= 16_384);
    let kept = lean["items"].as_array().map_or(0, Vec::len);
    assert!(kept < 43, "{kept} of 43 items kept");
    assert_eq!(lean["findings"], full["findings"]);
    assert_eq!(
        inspect(&mut host, &args(json!({ "budget": "big" })))
            .expect_err("budget")
            .code(),
        Some(codes::INVALID)
    );
}

#[test]
fn the_check_command_reports_the_findings_and_tells_the_panel() {
    let mut host = host(PAGE);
    view(&mut host, 1, URL);
    let answer = check_command(&mut host, &Map::new()).expect("answer");
    assert_eq!(answer["findings"], 1);
    assert_eq!(answer["errors"], 1);
    assert_eq!(answer["reported"], true);
    assert_eq!(answer[POST_KEY][0]["message"]["type"], "checked");
    assert_eq!(host.findings.len(), 1);
    assert_eq!(host.findings[0].view, Some(1));
}

#[test]
fn the_check_command_checks_the_view_its_target_names() {
    let mut host = host(PAGE);
    view(&mut host, 1, URL);
    view(&mut host, 2, URL);
    host.active = Some(1);
    let answer = check_command(&mut host, &args(json!({ "target": 2 }))).expect("answer");
    assert_eq!(answer["view"], 2);
    let evaluated: Vec<u32> = host.evaluated.iter().map(|(view, _)| *view).collect();
    assert_eq!(evaluated, [2]);
    assert_eq!(host.findings[0].view, Some(2));
    assert_eq!(
        check_command(&mut host, &args(json!({ "target": 9 })))
            .expect_err("gone")
            .code(),
        Some(codes::NOT_FOUND)
    );
}
