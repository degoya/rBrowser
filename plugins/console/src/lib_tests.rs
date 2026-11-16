use serde_json::json;

use super::*;
use crate::fake_host::{FakeHost, view};
use crate::feed::FEED_KEY;

fn host() -> FakeHost {
    FakeHost::with_views(vec![
        view(1, "Small", 320),
        view(2, "Mobile", 375),
        view(3, "Tablet", 768),
        view(4, "Desktop", 1280),
    ])
}

/// What the host captures from `tests/fixtures/site/console.html` in four views: one warning
/// in each, an uncaught error in the two below 600 px.
fn fixture_args(extra: Value) -> Value {
    let views: Vec<Value> = [(1, 320), (2, 375), (3, 768), (4, 1280)]
        .into_iter()
        .map(|(id, width)| {
            let at = f64::from(id) * 10.0;
            let mut view = json!({
                "view": id,
                "console": [{ "level": "warning", "source": "console-api",
                    "text": format!("console fixture: deprecated layout API (viewport {width})"),
                    "url": "http://127.0.0.1:8080/console.html", "line": 15, "time": at }],
            });
            if width < 600 {
                view["errors"] = json!([{ "text": format!("Uncaught Error: console fixture: layout failed at {width} px"),
                    "url": "http://127.0.0.1:8080/console.html", "line": 17, "column": 11, "time": at + 1.0 }]);
            }
            view
        })
        .collect();
    let mut args = extra;
    args[FEED_KEY] = json!({ "views": views });
    args
}

fn call(host: &mut FakeHost, tool: &str, args: &Value) -> Result<Value, Failure> {
    Console
        .call_tool(host, tool, &args.to_string())
        .map(|text| serde_json::from_str(&text).expect("the answer is JSON"))
}

fn command(host: &mut FakeHost, name: &str, args: &Value) -> Result<Value, Failure> {
    Console
        .run_command(host, name, &args.to_string())
        .map(|text| serde_json::from_str(&text).expect("the answer is JSON"))
}

#[test]
fn the_fixture_is_two_entries_the_error_only_in_the_narrow_views() {
    let mut host = host();
    let answer = call(&mut host, GROUPED, &fixture_args(json!({}))).expect("answers");
    assert_eq!(answer["captured"], true);
    assert_eq!(answer["total"], 2);
    let entries = answer["entries"].as_array().expect("a list");
    assert_eq!(entries[0]["level"], "warning");
    assert_eq!(entries[0]["views"], json!([1, 2, 3, 4]));
    assert_eq!(entries[1]["level"], "error");
    assert_eq!(entries[1]["kind"], "exception");
    assert_eq!(entries[1]["views"], json!([1, 2]));
    assert_eq!(answer["views"][0]["label"], "Small");
    assert_eq!(answer["views"].as_array().map(Vec::len), Some(4));
}

#[test]
fn without_the_hosts_capture_the_console_is_empty_and_says_so() {
    let mut host = host();
    let answer = call(&mut host, GROUPED, &json!({})).expect("answers");
    assert_eq!(answer["captured"], false);
    assert_eq!(answer["total"], 0);
}

#[test]
fn grouped_filters_by_level_view_target_and_text() {
    let mut host = host();
    let errors = fixture_args(json!({ "levels": ["error"] }));
    assert_eq!(
        call(&mut host, GROUPED, &errors).expect("answers")["total"],
        1
    );
    let wide = fixture_args(json!({ "views": [4] }));
    assert_eq!(
        call(&mut host, GROUPED, &wide).expect("answers")["total"],
        1
    );
    let target = fixture_args(json!({ "target": 3 }));
    assert_eq!(
        call(&mut host, GROUPED, &target).expect("answers")["total"],
        1
    );
    let search = fixture_args(json!({ "search": "LAYOUT FAILED" }));
    assert_eq!(
        call(&mut host, GROUPED, &search).expect("answers")["total"],
        1
    );
}

#[test]
fn grouped_refuses_arguments_it_cannot_read() {
    let mut host = host();
    for args in [
        json!({ "levels": ["fatal"] }),
        json!({ "levels": "error" }),
        json!({ "views": ["x"] }),
        json!({ "limit": 0 }),
        json!({ "search": 3 }),
    ] {
        let refused = call(&mut host, GROUPED, &args).expect_err("refused");
        assert_eq!(
            refused.code.as_deref(),
            Some("plugin.console.errors.bad_args")
        );
    }
    let refused = Console
        .call_tool(&mut host, GROUPED, "[1]")
        .expect_err("no object");
    assert_eq!(refused.kind, FailureKind::BadData);
}

#[test]
fn clear_after_answering_hides_what_was_there() {
    let mut host = host();
    let answer =
        call(&mut host, GROUPED, &fixture_args(json!({ "clear": true }))).expect("answers");
    assert_eq!(answer["total"], 2, "the answer still holds what it cleared");
    assert_eq!(answer["clearedAt"], 40.0);
    assert_eq!(answer["$post"][0]["target"], "panel");
    let after = call(&mut host, GROUPED, &fixture_args(json!({}))).expect("answers");
    assert_eq!(after["total"], 0);
}

#[test]
fn the_clear_command_marks_and_tells_the_panel() {
    let mut host = host();
    let answer = command(&mut host, CLEAR, &fixture_args(json!({}))).expect("clears");
    assert_eq!(answer["clearedAt"], 40.0);
    assert_eq!(answer["$post"][0]["message"]["cleared"], 40.0);
    let after = call(&mut host, GROUPED, &fixture_args(json!({}))).expect("answers");
    assert_eq!(after["total"], 0);
    // Without a capture it clears up to now.
    let answer = command(&mut host, CLEAR, &json!({})).expect("clears");
    assert_eq!(answer["clearedAt"], 1_731_300_001_000.0);
}

#[test]
fn the_exec_command_runs_for_a_person() {
    let mut host = host();
    for id in 1..=4 {
        host.answers.insert(
            id,
            Ok(json!({ "ok": true, "type": "string", "text": "\"Console fixture\"" }).to_string()),
        );
    }
    let answer =
        command(&mut host, EXEC, &json!({ "expression": "document.title" })).expect("runs");
    assert_eq!(answer["results"].as_array().map(Vec::len), Some(1));
    assert_eq!(answer["results"][0]["views"], json!([1, 2, 3, 4]));

    host.set("execTarget", r#""active""#);
    host.active = Some(2);
    host.evaluated.clear();
    command(&mut host, EXEC, &json!({ "expression": "1" })).expect("runs");
    assert_eq!(host.evaluated.len(), 1);
}

#[test]
fn the_exec_tool_is_refused_over_mcp_until_a_person_allows_it() {
    let mut host = host();
    let refused = call(
        &mut host,
        EXEC,
        &json!({ "expression": "document.title", "target": "all" }),
    )
    .expect_err("off by default");
    assert_eq!(
        refused.code.as_deref(),
        Some("plugin.console.errors.mcp_exec_off")
    );
    assert_eq!(refused.kind, FailureKind::AccessDenied);
    assert!(host.evaluated.is_empty());

    host.set("mcpExec", "true");
    let answer = call(&mut host, EXEC, &json!({ "expression": "1", "target": 3 })).expect("runs");
    assert_eq!(answer["results"][0]["views"], json!([3]));
    call(&mut host, EXEC, &json!({ "expression": "1" })).expect("all views without a target");
    assert_eq!(host.evaluated.len(), 5);
    let refused = call(
        &mut host,
        EXEC,
        &json!({ "expression": "1", "target": "active" }),
    )
    .expect_err("MCP names views");
    assert_eq!(
        refused.code.as_deref(),
        Some("plugin.console.errors.bad_args")
    );
}

#[test]
fn the_panel_command_asks_for_the_panel() {
    let mut host = host();
    let answer = command(&mut host, PANEL, &json!({})).expect("answers");
    assert_eq!(answer, json!({ OPEN_KEY: { "panel": PANEL_ID } }));
}

#[test]
fn unknown_contributions_are_refused() {
    let mut host = host();
    let refused = command(&mut host, "nope", &json!({})).expect_err("unknown");
    assert_eq!(refused.code.as_deref(), Some(codes::UNKNOWN_CONTRIBUTION));
    let refused = call(&mut host, "@page", &json!({})).expect_err("no page script");
    assert_eq!(refused.code.as_deref(), Some(codes::UNKNOWN_CONTRIBUTION));
}

#[test]
fn the_check_reports_js_errors_with_the_right_views() {
    let mut host = host();
    let mut all = Vec::new();
    for view in 1..=4 {
        let ctx = CheckContext {
            view: Some(view),
            url: None,
            rules: vec![
                findings::JS_ERROR.to_owned(),
                findings::CONSOLE_ERROR.to_owned(),
            ],
            args: fixture_args(json!({})).to_string(),
        };
        all.extend(Console.run(&mut host, &ctx).expect("runs"));
    }
    assert!(all.iter().all(|f| f.rule == findings::JS_ERROR));
    assert_eq!(
        all.iter().map(|f| f.view).collect::<Vec<_>>(),
        [Some(1), Some(2)]
    );
    assert_eq!(Console.describe().len(), 2);
    let empty = CheckContext {
        args: String::new(),
        ..CheckContext::default()
    };
    assert!(Console.run(&mut host, &empty).expect("runs").is_empty());
}
