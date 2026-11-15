use serde_json::{Value, json};

use super::*;
use crate::testing::FakeHost;

fn page_data() -> Value {
    json!({
        "tag": "button", "color": "rgb(119, 119, 119)",
        "backgrounds": [{ "color": "rgb(255, 255, 255)" }],
        "font": { "size": 16, "weight": 400 }, "role": "button", "name": "Send",
    })
}

fn run(host: &mut FakeHost, args: Value) -> Result<Value, rb_plugin_api::Failure> {
    inspect(host, &args.to_string()).map(|text| serde_json::from_str(&text).expect("JSON"))
}

#[test]
fn a_selector_in_one_view_is_inspected_there() {
    let mut host = FakeHost::default();
    host.evaluate.insert(2, page_data().to_string());
    let answer = run(&mut host, json!({ "target": 2, "selector": "#send" })).expect("answers");
    assert_eq!(answer["views"][0]["view"], 2);
    assert_eq!(answer["views"][0]["found"], true);
    assert_eq!(answer["views"][0]["inspection"]["contrast"]["ratio"], 4.48);
    assert_eq!(answer["views"].as_array().map(Vec::len), Some(1));
    let (view, script) = &host.evaluated[0];
    assert_eq!(*view, 2);
    assert!(script.contains("PeekCollect.inspect({\"selector\":\"#send\"})"));
    // The same selector reaches CDP.
    assert_eq!(host.cdp_sent[0].1, "Accessibility.getFullAXTree");
}

#[test]
fn all_views_are_inspected_without_a_target() {
    let mut host = FakeHost::default();
    host.view(1, "chromium").view(2, "firefox");
    host.evaluate.insert(1, page_data().to_string());
    host.evaluate.insert(2, "null".to_owned());
    let answer = run(&mut host, json!({ "target": "all", "x": 10, "y": 20.5 })).expect("answers");
    assert_eq!(answer["views"][0]["found"], true);
    assert_eq!(answer["views"][1], json!({ "view": 2, "found": false }));
    assert!(host.evaluated[0].1.contains("{\"x\":10.0,\"y\":20.5}"));
    // CDP only where an element was found, and never for a Firefox or WebKit view.
    assert!(host.cdp_sent.iter().all(|(view, _, _)| *view == 1));
    let answer = run(&mut host, json!({ "selector": "p" })).expect("answers");
    assert_eq!(answer["views"].as_array().map(Vec::len), Some(2));
}

#[test]
fn an_answer_as_a_json_string_is_read_too() {
    let mut host = FakeHost::default();
    host.evaluate
        .insert(1, Value::String(page_data().to_string()).to_string());
    let answer = run(&mut host, json!({ "target": 1, "selector": "p" })).expect("answers");
    assert_eq!(answer["views"][0]["inspection"]["tag"], "button");
}

#[test]
fn a_view_that_fails_names_its_error_and_the_others_go_on() {
    let mut host = FakeHost::default();
    host.view(1, "chromium").view(2, "chromium");
    host.evaluate.insert(2, page_data().to_string());
    let answer = run(&mut host, json!({ "selector": "p" })).expect("answers");
    assert_eq!(
        answer["views"][0],
        json!({ "view": 1, "found": false, "error": "plugins.error.not_found" })
    );
    assert_eq!(answer["views"][1]["found"], true);
}

#[test]
fn the_arguments_are_checked() {
    let mut host = FakeHost::default();
    for args in [
        json!({}),
        json!({ "selector": "p", "x": 1, "y": 2 }),
        json!({ "x": 1 }),
        json!({ "x": -1, "y": 2 }),
        json!({ "selector": "" }),
        json!({ "selector": "a".repeat(2000) }),
        json!({ "selector": 7 }),
        json!({ "target": "left", "selector": "p" }),
        json!("p"),
    ] {
        let failure = run(&mut host, args.clone()).expect_err("refused");
        assert_eq!(
            failure.code.as_deref(),
            Some(rb_plugin_api::codes::INVALID),
            "{args}"
        );
    }
}

#[test]
fn the_collector_script_is_one_expression() {
    let script = collector_script(&json!({ "selector": "p" }));
    assert!(script.starts_with("(() => {"));
    assert!(script.trim_end().ends_with("})()"));
    assert!(COLLECTOR.contains("PeekCollect"));
}
