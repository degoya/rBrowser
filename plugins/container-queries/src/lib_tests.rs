use rb_plugin_api::feature::Feature;
use rb_plugin_api::test_support::MemoryHost;
use rb_plugin_api::{Capability, FailureKind, ViewInfo};
use serde_json::{Value, json};

use super::*;
use crate::analysis::tests::fixture_report;
use crate::fake::PerView;
use crate::state::{STATE_FILE, STATE_PATH};

fn view(id: u32, width: u32) -> ViewInfo {
    ViewInfo {
        id,
        label: format!("{width}px"),
        breakpoint: format!("bp{width}"),
        engine: "chromium".to_owned(),
        width,
        height: 800,
        dpr: 1.0,
        url: "http://example.test/containers.html".to_owned(),
        title: "Containers".to_owned(),
    }
}

fn host() -> PerView {
    let mut host = MemoryHost::new()
        .grant(Capability::Views)
        .grant(Capability::Evaluate)
        .grant(Capability::ProjectRead)
        .grant(Capability::OutWrite);
    host.views = vec![view(1, 375), view(2, 1280)];
    host.active = Some(2);
    PerView {
        host,
        ..PerView::default()
    }
}

/// What `out.write` wrote becomes readable again, as the real project folder does.
fn sync(host: &mut PerView) {
    if let Some(bytes) = host.host.out.get(STATE_FILE).cloned() {
        host.host.project.insert(STATE_PATH.to_owned(), bytes);
    }
}

fn command(host: &mut PerView, command: &str, args: Value) -> Result<Value, Failure> {
    let answer = ContainerQueries.run_command(host, command, &args.to_string());
    sync(host);
    answer.map(|text| serde_json::from_str(&text).expect("JSON"))
}

fn tool(host: &mut PerView, tool: &str, args: Value) -> Result<Value, Failure> {
    let answer = ContainerQueries.call_tool(host, tool, &args.to_string());
    sync(host);
    answer.map(|text| serde_json::from_str(&text).expect("JSON"))
}

fn hello(host: &mut PerView, view: u32) -> Value {
    tool(
        host,
        PAGE_TOOL,
        json!({ "view": view, "message": { "type": "hello" } }),
    )
    .expect("hello")
}

#[test]
fn the_command_toggles_the_overlay_in_every_view_and_posts_it() {
    let mut host = host();
    let on = command(&mut host, "overlay", json!({})).expect("on");
    assert_eq!(on["on"], json!(true));
    assert_eq!(on["target"], json!("all"));
    let posts = on["$post"].as_array().expect("posts");
    assert_eq!(posts[0]["target"], json!("all"));
    assert_eq!(
        posts[0]["message"],
        json!({ "type": "state", "on": true, "settings": { "labels": true, "color": "#0d9488" } })
    );
    assert_eq!(posts[1]["target"], json!("panel"));
    assert_eq!(hello(&mut host, 1)["on"], json!(true));
    let off = command(&mut host, "overlay", json!({})).expect("off");
    assert_eq!(off["on"], json!(false));
    assert_eq!(hello(&mut host, 2)["on"], json!(false));
}

#[test]
fn the_tool_switches_one_view_and_the_active_one() {
    let mut host = host();
    let on = tool(&mut host, "overlay", json!({ "on": true, "target": 1 })).expect("on");
    assert_eq!(on["$post"][0]["target"], json!(1));
    assert_eq!(hello(&mut host, 1)["on"], json!(true));
    assert_eq!(hello(&mut host, 2)["on"], json!(false));
    let active = tool(
        &mut host,
        "overlay",
        json!({ "on": true, "target": "active" }),
    )
    .expect("on");
    assert_eq!(active["target"], json!(2));
    assert_eq!(hello(&mut host, 2)["on"], json!(true));
    let bad = tool(&mut host, "overlay", json!({ "on": "yes" })).expect_err("bad");
    assert_eq!(bad.kind, FailureKind::BadData);
}

#[test]
fn the_hello_carries_the_settings() {
    let mut host = host();
    host.host
        .settings
        .insert("overlay.labels".to_owned(), "false".to_owned());
    assert_eq!(hello(&mut host, 1)["settings"]["labels"], json!(false));
    let other = tool(
        &mut host,
        PAGE_TOOL,
        json!({ "view": 1, "message": { "type": "nope" } }),
    );
    assert!(other.is_err());
}

#[test]
fn analyze_evaluates_every_view_and_merges_the_fixture() {
    let mut host = host();
    host.answers.insert(1, fixture_report(280.0).to_string());
    host.answers.insert(2, fixture_report(640.0).to_string());
    let analysis = tool(&mut host, "analyze", json!({})).expect("analysis");
    assert_eq!(analysis["conditions"].as_array().map(Vec::len), Some(4));
    assert_eq!(analysis["containers"].as_array().map(Vec::len), Some(3));
    assert_eq!(
        analysis["containers"][1]["switches"],
        json!([{ "from": 1, "to": 2, "gained": [0], "lost": [1] }])
    );
    assert_eq!(host.host.evaluated.len(), 2);
    assert!(
        host.host.evaluated[0]
            .1
            .contains("__rbContainers.analyze()")
    );
    assert_eq!(analysis["overlay"], json!(false));
    tool(&mut host, "overlay", json!({ "on": true })).expect("on");
    let again = tool(&mut host, "analyze", json!({})).expect("analysis");
    assert_eq!(again["overlay"], json!(true));
}

#[test]
fn analyze_names_a_view_whose_answer_is_no_report() {
    let mut host = host();
    host.answers.insert(1, "\"text\"".to_owned());
    host.answers.insert(2, fixture_report(640.0).to_string());
    let analysis = tool(&mut host, "analyze", json!({ "target": "active" })).expect("analysis");
    assert_eq!(analysis["views"].as_array().map(Vec::len), Some(1));
    assert_eq!(analysis["views"][0]["view"], json!(2));
    let all = tool(&mut host, "analyze", json!({})).expect("analysis");
    assert_eq!(all["views"][0]["error"], json!(codes::BAD_OUTPUT));
}

#[test]
fn the_analyze_command_opens_the_panel_without_evaluating() {
    let mut host = host();
    let answer = command(&mut host, "analyze", json!({})).expect("answer");
    assert_eq!(answer["$open"], json!({ "panel": "main" }));
    assert_eq!(answer["$post"][0]["message"], json!({ "type": "refresh" }));
    assert!(host.host.evaluated.is_empty());
}

#[test]
fn resize_pins_a_width_in_the_target_and_reset_brings_it_back() {
    let mut host = host();
    host.answers.insert(1, "true".to_owned());
    let resized = tool(
        &mut host,
        "resize",
        json!({ "path": "#hero", "width": 400, "target": 1 }),
    )
    .expect("resized");
    assert_eq!(resized["views"], json!([{ "view": 1, "resized": true }]));
    assert_eq!(resized["$post"][0]["message"], json!({ "type": "refresh" }));
    assert!(
        host.host.evaluated[0]
            .1
            .contains(r##"resize("#hero", 400)"##)
    );
    host.host.evaluate_answer = "2".to_owned();
    host.answers.clear();
    let reset = tool(&mut host, "resize", json!({ "reset": true })).expect("reset");
    assert_eq!(
        reset["views"],
        json!([{ "view": 1, "reset": 2 }, { "view": 2, "reset": 2 }])
    );
    assert!(host.host.evaluated[1].1.contains("reset(null)"));
}

#[test]
fn resize_refuses_what_it_cannot_set() {
    let mut host = host();
    for args in [
        json!({ "path": "#hero" }),
        json!({ "width": 400 }),
        json!({ "path": "#hero", "width": -1 }),
        json!({ "path": "#hero", "width": 100_001 }),
        json!({ "path": "", "width": 10 }),
        json!({ "path": "x".repeat(301), "width": 10 }),
        json!({ "path": 3, "width": 10 }),
        json!({ "reset": "yes" }),
    ] {
        let failure = tool(&mut host, "resize", args.clone()).expect_err("refused");
        assert_eq!(failure.kind, FailureKind::BadData, "{args}");
    }
    assert!(host.host.evaluated.is_empty());
    host.host.evaluate_answer = "{}".to_owned();
    let odd = tool(
        &mut host,
        "resize",
        json!({ "path": "#a", "width": 1, "target": 2 }),
    )
    .expect("answer");
    assert_eq!(odd["views"][0]["error"], json!(codes::BAD_OUTPUT));
}

#[test]
fn a_start_forgets_single_views_and_keeps_every_views() {
    let mut host = host();
    tool(&mut host, "overlay", json!({ "on": true })).expect("on");
    tool(&mut host, "overlay", json!({ "on": false, "target": 1 })).expect("off");
    ContainerQueries.activate(&mut host).expect("activated");
    sync(&mut host);
    assert_eq!(hello(&mut host, 1)["on"], json!(true));
}

#[test]
fn unknown_contributions_are_not_found() {
    let mut host = host();
    let tool_failure = tool(&mut host, "nope", json!({})).expect_err("unknown");
    assert_eq!(tool_failure.kind, FailureKind::NotFound);
    let command_failure = command(&mut host, "nope", json!({})).expect_err("unknown");
    assert_eq!(command_failure.code(), Some(codes::UNKNOWN_CONTRIBUTION));
}
