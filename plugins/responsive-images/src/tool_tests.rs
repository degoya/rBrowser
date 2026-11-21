use rb_plugin_api::{RULE_LEVELS_SETTING, codes};
use serde_json::{Value, json};

use super::*;
use crate::fake::FakeHost;

const MOBILE: &str = include_str!("../tests/fixtures/mobile.json");
const TABLET: &str = include_str!("../tests/fixtures/tablet.json");

fn fixture(text: &str) -> Value {
    serde_json::from_str(text).expect("fixture")
}

fn host() -> FakeHost {
    let mut host = FakeHost::default();
    host.view(1, "Mobile", "https://example.test/")
        .shaped("chromium", 390, 3.0)
        .page(1, &fixture(MOBILE));
    host.view(2, "Tablet", "https://example.test/")
        .shaped("chromium", 768, 2.0)
        .page(2, &fixture(TABLET));
    host.view(3, "Firefox", "https://example.test/")
        .shaped("firefox", 1280, 1.0);
    host
}

fn args(value: Value) -> Map<String, Value> {
    value.as_object().cloned().unwrap_or_default()
}

#[test]
fn arguments_are_an_object() {
    assert!(parse("").expect("empty").is_empty());
    assert!(parse("null").expect("null").is_empty());
    assert_eq!(parse(r#"{"target": 2}"#).expect("object")["target"], 2);
    let failure = parse("[1]").expect_err("array");
    assert_eq!(failure.code(), Some(codes::INVALID));
}

#[test]
fn check_answers_every_view_with_its_images_and_issues() {
    let mut host = host();
    let answer = check(&mut host, &Map::new()).expect("answer");
    let views = answer["views"].as_array().expect("views");
    assert_eq!(views.len(), 3);
    let mobile = &views[0];
    assert_eq!(mobile["label"], "Mobile");
    assert_eq!(mobile["dpr"], 3.0);
    assert_eq!(mobile["images"].as_array().map(Vec::len), Some(5));
    let hero = &mobile["images"][0];
    assert_eq!(hero["selector"], "#hero");
    assert_eq!(hero["lcp"], true);
    assert_eq!(hero["issues"][0]["rule"], "lcp");
    assert_eq!(hero["issues"][0]["severity"], "warning");
    assert_eq!(
        hero["issues"][0]["message"],
        "plugin.responsive-images.findings.lcp.lazy"
    );
    assert_eq!(hero["issues"][0]["params"]["dpr"], "3");
    assert_eq!(views[2]["error"]["code"], codes::NOT_AVAILABLE);
    assert!(views[2].get("images").is_none());
    assert_eq!(
        answer["summary"],
        json!({ "error": 0, "warning": 8, "info": 0 })
    );
    assert_eq!(answer["settings"]["oversizeFactor"], 1.5);
    assert_eq!(answer["cut"], false);
    assert!(host.findings.is_empty(), "the tool reports nothing");
}

#[test]
fn check_takes_one_view_and_the_person_levels() {
    let mut host = host();
    host.settings.insert(
        RULE_LEVELS_SETTING.to_owned(),
        r#"{ "oversized": "error", "lcp": "off" }"#.to_owned(),
    );
    let answer = check(&mut host, &args(json!({ "target": 2 }))).expect("answer");
    let views = answer["views"].as_array().expect("views");
    assert_eq!(views.len(), 1);
    let rules: Vec<(&str, &str)> = views[0]["images"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|image| image["issues"].as_array().into_iter().flatten())
        .map(|issue| {
            (
                issue["rule"].as_str().unwrap_or_default(),
                issue["severity"].as_str().unwrap_or_default(),
            )
        })
        .collect();
    assert_eq!(
        rules,
        [
            ("oversized", "error"),
            ("undersized", "warning"),
            ("sizes-missing", "warning")
        ]
    );
    for bad in [json!({ "target": "x" }), json!({ "budget": "big" })] {
        let failure = check(&mut host, &args(bad.clone())).expect_err("bad");
        assert_eq!(failure.code(), Some(codes::INVALID), "{bad}");
    }
}

#[test]
fn a_small_budget_drops_the_raw_texts_then_the_images_without_issues() {
    let mut host = host();
    let full = check(&mut host, &Map::new()).expect("answer");
    let roomy = fitted(full.clone(), Some(u64::MAX));
    assert_eq!(roomy, full);
    let tight = fitted(full, Some(4096));
    assert_eq!(tight["cut"], true);
    let mobile = &tight["views"][0];
    assert_eq!(mobile["hidden"], 1, "the correctly sized image");
    let images = mobile["images"].as_array().expect("images");
    assert_eq!(images.len(), 4);
    assert!(images.iter().all(|image| image.get("srcset").is_none()));
    assert!(tight["views"][2].get("hidden").is_none());
}

#[test]
fn the_command_reports_every_view_and_tells_the_panel() {
    let mut host = host();
    let answer = check_command(&mut host, &Map::new()).expect("checked");
    assert_eq!(answer["views"], 2);
    assert_eq!(answer["findings"], 8);
    assert_eq!(answer["reported"], true);
    assert_eq!(host.findings.len(), 8);
    assert_eq!(answer[POST_KEY][0]["message"]["type"], "checked");
    let one = check_command(&mut host, &args(json!({ "target": 1 }))).expect("one");
    assert_eq!(one["findings"], 4);
    let failure = check_command(&mut host, &args(json!({ "target": 3 }))).expect_err("firefox");
    assert_eq!(failure.code(), Some(codes::NOT_AVAILABLE));
    host.refuse_findings = true;
    let refused = check_command(&mut host, &Map::new()).expect("answered");
    assert_eq!(refused["reported"], false);
}

#[test]
fn mark_highlights_in_the_views_named_and_clears_everywhere() {
    let mut host = host();
    host.cdp.insert(
        "DOM.getDocument".to_owned(),
        Ok(json!({ "root": { "nodeId": 1 } })),
    );
    host.cdp
        .insert("DOM.querySelector".to_owned(), Ok(json!({ "nodeId": 4 })));
    let answer = mark(
        &mut host,
        &args(json!({ "targets": [
            { "view": 1, "selector": "#hero" },
            { "view": 2, "selector": "#hero" },
            { "view": 9, "selector": "#hero" }
        ] })),
    )
    .expect("marked");
    assert_eq!(
        answer["marked"],
        json!([
            { "view": 1, "found": true },
            { "view": 2, "found": true },
            { "view": 9, "found": false, "error": codes::NOT_FOUND }
        ])
    );
    assert!(host.methods().contains(&"Overlay.highlightNode"));
    assert!(host.evaluated.is_empty(), "no script");
    host.cdp_sent.clear();
    let cleared = mark(&mut host, &args(json!({ "clear": true }))).expect("cleared");
    assert_eq!(cleared["cleared"], true);
    assert_eq!(host.methods(), ["Overlay.hideHighlight"; 3]);
    let failure = mark(&mut host, &Map::new()).expect_err("no targets");
    assert_eq!(failure.code(), Some(codes::INVALID));
}
