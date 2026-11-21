use rb_plugin_api::test_support::MemoryHost;

use super::*;

#[test]
fn a_tool_asks_the_core_for_its_op_while_offered() {
    let offered = |extra: Value| {
        let mut args = extra;
        args[OFFER_KEY] = json!("tool");
        args
    };
    assert_eq!(
        tool_answer(COLLECTIONS, &offered(json!({}))).expect("asks"),
        json!({ OFFER_KEY: { "op": "list" } })
    );
    assert_eq!(
        tool_answer(COLLECTIONS, &offered(json!({ "id": "users/me.http" }))).expect("asks"),
        json!({ OFFER_KEY: { "op": "load", "id": "users/me.http" } })
    );
    assert_eq!(
        tool_answer(
            RUN,
            &offered(json!({ "id": "me.http", "environment": "staging" }))
        )
        .expect("asks"),
        json!({ OFFER_KEY: { "op": "run", "id": "me.http", "environment": "staging" } })
    );
    assert_eq!(
        tool_answer(HISTORY, &offered(json!({}))).expect("asks"),
        json!({ OFFER_KEY: { "op": "history" } })
    );
    let op = json!({ "op": "send", "request": { "method": "GET", "url": "https://a.example" } });
    assert_eq!(
        tool_answer(PANEL_TOOL, &offered(json!({ "op": op.clone() }))).expect("asks"),
        json!({ OFFER_KEY: op })
    );
}

#[test]
fn code_asks_for_a_saved_request_in_a_language() {
    let args = json!({ "id": "me.http", "language": "curl", OFFER_KEY: "tool" });
    assert_eq!(
        tool_answer(CODE, &args).expect("asks"),
        json!({ OFFER_KEY: { "op": "code", "id": "me.http", "language": "curl", "environment": null } })
    );
    let failure =
        tool_answer(CODE, &json!({ "id": "me.http", OFFER_KEY: "tool" })).expect_err("no language");
    assert_eq!(
        failure.params.get("reason").map(String::as_str),
        Some("language")
    );
}

#[test]
fn the_second_call_hands_the_result_back() {
    let args = json!({ "id": "me.http", RESULT_KEY: { "status": 200 } });
    assert_eq!(
        tool_answer(RUN, &args).expect("answers"),
        json!({ "status": 200 })
    );
}

#[test]
fn without_the_core_nothing_is_asked() {
    let failure = tool_answer(RUN, &json!({ "id": "me.http" })).expect_err("refused");
    assert_eq!(failure.code(), Some("plugin.api-client.errors.no_core"));
}

#[test]
fn bad_arguments_and_unknown_tools_are_refused() {
    let offered = json!({ OFFER_KEY: "tool" });
    let failure = tool_answer(RUN, &offered).expect_err("no id");
    assert_eq!(failure.code(), Some("plugin.api-client.errors.bad_args"));
    let failure = tool_answer(RUN, &json!({ OFFER_KEY: "tool", "id": 3 })).expect_err("no text");
    assert_eq!(failure.code(), Some("plugin.api-client.errors.bad_args"));
    let failure = tool_answer(PANEL_TOOL, &json!({ OFFER_KEY: "panel", "op": "send" }))
        .expect_err("no object");
    assert_eq!(failure.code(), Some("plugin.api-client.errors.bad_args"));
    let failure = tool_answer("format", &offered).expect_err("unknown");
    assert_eq!(failure.code(), Some(codes::UNKNOWN_CONTRIBUTION));
    assert!(parse("[1]").is_err());
    assert_eq!(parse(" ").expect("empty"), json!({}));
}

#[test]
fn the_commands_show_the_panel_and_send_its_request() {
    let mut host = MemoryHost::default();
    let shown = ApiClient
        .run_command(&mut host, PANEL, "{}")
        .expect("shows");
    assert_eq!(
        serde_json::from_str::<Value>(&shown).expect("json"),
        json!({ OPEN_KEY: { "panel": "api-client" } })
    );
    let sent = ApiClient.run_command(&mut host, SEND, "{}").expect("sends");
    let sent: Value = serde_json::from_str(&sent).expect("json");
    assert_eq!(sent["$post"][0]["message"], json!({ "send": true }));
    assert!(ApiClient.run_command(&mut host, "other", "{}").is_err());
}

#[test]
fn a_collection_run_its_report_and_a_benchmark_are_asked_for_saved_requests() {
    let offered = |extra: Value| {
        let mut args = extra;
        args[OFFER_KEY] = json!("tool");
        args
    };
    assert_eq!(
        tool_answer(RUN_COLLECTION, &offered(json!({ "folder": "users" }))).expect("asks"),
        json!({ OFFER_KEY: { "op": "runCollection", "folder": "users", "environment": null } })
    );
    assert_eq!(
        tool_answer(REPORT, &offered(json!({}))).expect("asks"),
        json!({ OFFER_KEY: { "op": "report" } })
    );
    assert_eq!(
        tool_answer(
            BENCH,
            &offered(json!({ "id": "me.http", "environments": ["staging", "production"] }))
        )
        .expect("asks"),
        json!({ OFFER_KEY: {
            "op": "bench", "id": "me.http", "environments": ["staging", "production"]
        } })
    );
    assert_eq!(
        tool_answer(BENCH, &offered(json!({ "id": "me.http" }))).expect("asks"),
        json!({ OFFER_KEY: { "op": "bench", "id": "me.http", "environments": [] } })
    );
    for bad in [
        json!({}),
        json!({ "id": "me.http", "environments": "staging" }),
        json!({ "id": "me.http", "environments": [1] }),
    ] {
        let failure = tool_answer(BENCH, &offered(bad)).expect_err("refused");
        assert_eq!(failure.code(), Some("plugin.api-client.errors.bad_args"));
    }
}

#[test]
fn the_run_command_has_the_panel_run_the_collections() {
    let mut host = MemoryHost::default();
    let run = ApiClient
        .run_command(&mut host, RUN_COLLECTION_COMMAND, "{}")
        .expect("runs");
    let run: Value = serde_json::from_str(&run).expect("json");
    assert_eq!(run[OPEN_KEY], json!({ "panel": "api-client" }));
    assert_eq!(run["$post"][0]["message"], json!({ "runCollection": true }));
}
