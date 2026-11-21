use rb_plugin_api::test_support::MemoryHost;

use super::*;

fn offered(origin: &str, extra: Value) -> Value {
    let mut args = extra;
    args[OFFER_KEY] = json!(origin);
    args
}

#[test]
fn mcp_tools_ask_only_for_the_reports() {
    assert_eq!(
        tool_answer(REPORTS, &offered("tool", json!({}))).expect("asks"),
        json!({ OFFER_KEY: { "op": "reports" } })
    );
    assert_eq!(
        tool_answer(REPORT, &offered("tool", json!({ "id": "1-smoke" }))).expect("asks"),
        json!({ OFFER_KEY: { "op": "report", "id": "1-smoke" } })
    );
    assert_eq!(
        tool_answer(REPORT, &offered("tool", json!({}))).expect("asks"),
        json!({ OFFER_KEY: { "op": "report", "id": null } })
    );
    assert_eq!(
        tool_answer(COMPARE, &offered("tool", json!({ "a": "1-a", "b": "2-a" }))).expect("asks"),
        json!({ OFFER_KEY: { "op": "compare", "a": "1-a", "b": "2-a" } })
    );
}

#[test]
fn the_panel_forwards_its_op_and_the_second_call_hands_the_result_back() {
    let op = json!({ "op": "preview", "id": "smoke.json" });
    assert_eq!(
        tool_answer(PANEL_TOOL, &offered("panel", json!({ "op": op.clone() }))).expect("asks"),
        json!({ OFFER_KEY: op })
    );
    let args = json!({ RESULT_KEY: { "users": 50 } });
    assert_eq!(
        tool_answer(PANEL_TOOL, &args).expect("answers"),
        json!({ "users": 50 })
    );
}

#[test]
fn without_the_core_nothing_is_asked_and_bad_arguments_are_refused() {
    let failure = tool_answer(REPORTS, &json!({})).expect_err("no core");
    assert_eq!(failure.code(), Some("plugin.load-test.errors.no_core"));
    let failure = tool_answer(COMPARE, &offered("tool", json!({ "a": "1-a" }))).expect_err("b");
    assert_eq!(failure.code(), Some("plugin.load-test.errors.bad_args"));
    let failure = tool_answer(REPORT, &offered("tool", json!({ "id": 3 }))).expect_err("text");
    assert_eq!(failure.code(), Some("plugin.load-test.errors.bad_args"));
    let failure =
        tool_answer(PANEL_TOOL, &offered("panel", json!({ "op": "start" }))).expect_err("object");
    assert_eq!(failure.code(), Some("plugin.load-test.errors.bad_args"));
    let failure = tool_answer("start", &offered("tool", json!({}))).expect_err("unknown");
    assert_eq!(failure.code(), Some(codes::UNKNOWN_CONTRIBUTION));
    assert!(parse("[1]").is_err());
    assert_eq!(parse(" ").expect("empty"), json!({}));
}

#[test]
fn the_commands_show_the_panel_and_stop_the_run() {
    let mut host = MemoryHost::default();
    let shown = LoadTest.run_command(&mut host, PANEL, "{}").expect("shows");
    assert_eq!(
        serde_json::from_str::<Value>(&shown).expect("json"),
        json!({ OPEN_KEY: { "panel": "load-test" } })
    );
    let args = offered("command", json!({})).to_string();
    let stop = LoadTest
        .run_command(&mut host, CANCEL, &args)
        .expect("asks");
    assert_eq!(
        serde_json::from_str::<Value>(&stop).expect("json"),
        json!({ OFFER_KEY: { "op": "stop" } })
    );
    let done = json!({ RESULT_KEY: { "stopping": true } }).to_string();
    let told = LoadTest
        .run_command(&mut host, CANCEL, &done)
        .expect("told");
    let told: Value = serde_json::from_str(&told).expect("json");
    assert_eq!(
        told["$post"][0]["message"],
        json!({ "stopped": { "stopping": true } })
    );
    assert!(LoadTest.run_command(&mut host, CANCEL, "{}").is_err());
    assert!(LoadTest.run_command(&mut host, "start", "{}").is_err());
}
