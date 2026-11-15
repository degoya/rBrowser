//! The fixture over every view (RB-060-02): `tests/fixtures/site/console.html` in four views as
//! the host captures it, run through the plugin's native traits against `MemoryHost` - the
//! host's rules for the capabilities, the project paths and the out folder.

use console_plugin::Console;
use console_plugin::feed::FEED_KEY;
use rb_plugin_api::check::Check;
use rb_plugin_api::feature::Feature;
use rb_plugin_api::test_support::MemoryHost;
use rb_plugin_api::{Capability, CheckContext, ViewInfo};
use serde_json::{Value, json};

const FIXTURE: &str = include_str!("../../../tests/fixtures/site/console.html");
const PAGE: &str = "http://127.0.0.1:8080/console.html";
const WIDTHS: [(u32, u32); 4] = [(1, 320), (2, 375), (3, 768), (4, 1280)];

fn view(id: u32, width: u32) -> ViewInfo {
    ViewInfo {
        id,
        label: format!("View {width}"),
        breakpoint: format!("w{width}"),
        engine: "chromium".to_owned(),
        width,
        height: 800,
        dpr: 1.0,
        url: PAGE.to_owned(),
        title: "Console fixture".to_owned(),
    }
}

fn host() -> MemoryHost {
    let mut host = MemoryHost::new()
        .grant(Capability::Views)
        .grant(Capability::Evaluate)
        .grant(Capability::ProjectRead)
        .grant(Capability::OutWrite);
    host.views = WIDTHS.iter().map(|&(id, width)| view(id, width)).collect();
    host.now = 1_731_300_000;
    host
}

/// What the page logs in a view `width` pixels wide, as the host captures it.
fn capture() -> Value {
    let views: Vec<Value> = WIDTHS
        .iter()
        .map(|&(id, width)| {
            let at = 1_731_300_000_000.0 + f64::from(id);
            let mut view = json!({ "view": id, "navigatedAt": at - 500.0, "console": [{
                "level": "warning", "source": "console-api",
                "text": format!("console fixture: deprecated layout API (viewport {width})"),
                "url": PAGE, "line": 14, "time": at }] });
            if width < 600 {
                view["errors"] = json!([{
                    "text": format!("Uncaught Error: console fixture: layout failed at {width} px"),
                    "url": PAGE, "line": 18, "column": 37, "time": at + 0.5 }]);
            }
            view
        })
        .collect();
    json!({ FEED_KEY: { "views": views } })
}

fn grouped(host: &mut MemoryHost, extra: &Value) -> Value {
    let mut args = capture();
    if let (Some(args), Some(extra)) = (args.as_object_mut(), extra.as_object()) {
        args.extend(extra.clone());
    }
    let answer = Console
        .call_tool(host, "grouped", &args.to_string())
        .expect("grouped answers");
    serde_json::from_str(&answer).expect("JSON")
}

#[test]
fn the_fixture_logs_what_the_capture_says() {
    assert!(FIXTURE.contains("console.warn('console fixture: deprecated layout API (viewport '"));
    assert!(FIXTURE.contains("window.innerWidth < 600"));
    assert!(FIXTURE.contains("new Error('console fixture: layout failed at '"));
}

#[test]
fn two_entries_the_error_with_the_narrow_views_only() {
    let mut host = host();
    let answer = grouped(&mut host, &json!({}));
    assert_eq!(answer["total"], 2);
    assert_eq!(answer["entries"][0]["level"], "warning");
    assert_eq!(answer["entries"][0]["views"], json!([1, 2, 3, 4]));
    assert_eq!(answer["entries"][1]["level"], "error");
    assert_eq!(answer["entries"][1]["views"], json!([1, 2]));
    assert_eq!(answer["entries"][1]["source"]["line"], 18);
}

#[test]
fn a_clear_lands_in_the_out_folder() {
    let mut host = host();
    let answer = grouped(&mut host, &json!({ "clear": true }));
    let state = host
        .out
        .get("state.json")
        .expect("the clear mark is written");
    let state: Value = serde_json::from_slice(state).expect("JSON");
    assert_eq!(state["clearedAt"], answer["clearedAt"]);
    // The host serves the out folder below `.rbrowser/`: the next call reads the mark back.
    host.project.insert(
        ".rbrowser/out/console/state.json".to_owned(),
        host.out["state.json"].clone(),
    );
    assert_eq!(grouped(&mut host, &json!({}))["total"], 0);
}

#[test]
fn a_command_gives_one_result_for_equal_values() {
    let mut host = host();
    host.evaluate_answer =
        json!({ "ok": true, "type": "string", "text": "\"Console fixture\"" }).to_string();
    let answer = Console
        .run_command(&mut host, "exec", r#"{"expression":"document.title"}"#)
        .expect("runs");
    let answer: Value = serde_json::from_str(&answer).expect("JSON");
    assert_eq!(answer["results"].as_array().map(Vec::len), Some(1));
    assert_eq!(answer["results"][0]["views"], json!([1, 2, 3, 4]));
    assert_eq!(host.evaluated.len(), 4);
}

#[test]
fn checks_run_reports_js_errors_for_the_narrow_views() {
    let mut host = host();
    let rules: Vec<String> = Console.describe().into_iter().map(|rule| rule.id).collect();
    let mut found = Vec::new();
    for &(id, _) in &WIDTHS {
        let ctx = CheckContext {
            view: Some(id),
            url: Some(PAGE.to_owned()),
            rules: rules.clone(),
            args: capture().to_string(),
        };
        found.extend(Console.run(&mut host, &ctx).expect("runs"));
    }
    assert_eq!(found.len(), 2);
    assert!(found.iter().all(|finding| finding.rule == "js-error"));
    assert_eq!(
        found.iter().map(|f| f.view).collect::<Vec<_>>(),
        [Some(1), Some(2)]
    );
    assert_eq!(found[0].path, found[1].path);
    assert_eq!(found[0].url.as_deref(), Some(PAGE));
}

#[test]
fn a_host_that_grants_nothing_still_gets_an_answer() {
    let mut host = MemoryHost::new();
    let answer = grouped(&mut host, &json!({ "clear": true }));
    assert_eq!(answer["total"], 2);
    assert_eq!(answer["views"], json!([]));
    assert!(host.logs.iter().any(|(level, _)| level == "warn"));
}
