use std::sync::Mutex;

use rmcp::model::ContentBlock;

use super::*;

/// Records each call and answers per tool: `Ok(json)` as a text block, `Err((code, message))`
/// as the error result a guarded tool gives.
#[derive(Default)]
struct Fake {
    calls: Mutex<Vec<(String, Value)>>,
}

#[async_trait]
impl Dispatch for Fake {
    fn has_tool(&self, tool: &str) -> bool {
        tool != "missing"
    }

    async fn dispatch(
        &self,
        tool: &str,
        arguments: Map<String, Value>,
    ) -> Result<CallToolResponse, ErrorData> {
        rb_util::lock(&self.calls).push((tool.to_owned(), Value::Object(arguments)));
        match tool {
            "evaluate" => Ok(crate::answer::error_result(
                &crate::ToolFailure::new("evaluate.disabled", "evaluate is switched off")
                    .with_setting("mcp.allowEvaluate"),
            )
            .into()),
            "missing" => Err(ErrorData::invalid_params("tool not found", None)),
            "typed" => Err(ErrorData::invalid_params("width: expected u32", None)),
            "absent" => Err(ErrorData::new(
                rmcp::model::ErrorCode::METHOD_NOT_FOUND,
                "gone",
                None,
            )),
            "broken" => Err(ErrorData::internal_error(
                "the view crashed, not found",
                None,
            )),
            "screenshot" => Ok(CallToolResult::success(vec![
                ContentBlock::text(r#"{"views":2}"#),
                ContentBlock::image("AAAA", "image/png"),
            ])
            .into()),
            "note" => Ok(CallToolResult::success(vec![ContentBlock::text("plain words")]).into()),
            _ => Ok(CallToolResult::success(vec![ContentBlock::text(r#"{"ok":true}"#)]).into()),
        }
    }
}

fn parsed(text: &str) -> Value {
    serde_json::from_str(text).expect("JSON")
}

#[tokio::test]
async fn a_call_runs_the_tool_with_its_arguments_and_answers_its_json() {
    let bridge = Bridge::new(Fake::default(), 10);
    let answer = bridge
        .call("navigate", r#"{"target":"all","url":"https://a.test/"}"#)
        .await
        .expect("an answer");
    assert_eq!(parsed(&answer), json!({ "ok": true }));
    let calls = rb_util::lock(&bridge.dispatch.calls).clone();
    assert_eq!(
        calls,
        [(
            "navigate".to_owned(),
            json!({ "target": "all", "url": "https://a.test/" })
        )]
    );
    assert_eq!(bridge.calls(), 1);
}

#[tokio::test]
async fn a_guarded_tool_refuses_inside_a_script_as_it_does_outside() {
    let bridge = Bridge::new(Fake::default(), 10);
    let refusal = bridge
        .call("evaluate", r#"{"expression":"1"}"#)
        .await
        .expect_err("refused");
    let refusal = parsed(&refusal);
    assert_eq!(refusal["code"], "evaluate.disabled");
    // RB-099-16: a script written against the flat codes still finds them until 1.0.
    assert_eq!(refusal["legacyCode"], "evaluate_disabled");
    assert_eq!(refusal["message"], "evaluate is switched off");
}

#[tokio::test]
async fn script_run_is_refused_inside_a_script_without_reaching_the_router() {
    let bridge = Bridge::new(Fake::default(), 10);
    let refusal = bridge.call(TOOL, "{}").await.expect_err("refused");
    assert_eq!(parsed(&refusal)["code"], codes::RECURSION);
    assert!(rb_util::lock(&bridge.dispatch.calls).is_empty());
    assert_eq!(bridge.calls(), 0);
}

#[tokio::test]
async fn the_call_after_the_limit_is_refused_and_marks_the_run() {
    let bridge = Bridge::new(Fake::default(), 2);
    for _ in 0..2 {
        bridge
            .call("views_list", "{}")
            .await
            .expect("within the limit");
    }
    assert!(!bridge.exceeded());
    let refusal = bridge.call("views_list", "{}").await.expect_err("over");
    assert_eq!(parsed(&refusal)["code"], codes::CALL_LIMIT);
    assert!(bridge.exceeded());
    assert_eq!(bridge.calls(), 2);
}

#[tokio::test]
async fn an_unknown_tool_and_bad_arguments_are_refused_with_a_code() {
    let bridge = Bridge::new(Fake::default(), 10);
    let missing = bridge.call("missing", "{}").await.expect_err("unknown");
    assert_eq!(parsed(&missing)["code"], "tool.not_found");
    assert!(
        rb_util::lock(&bridge.dispatch.calls).is_empty(),
        "an unknown tool never reaches the router"
    );
    let bad = bridge
        .call("navigate", "[1, 2]")
        .await
        .expect_err("no object");
    assert_eq!(parsed(&bad)["code"], "args.invalid");
}

#[tokio::test]
async fn several_blocks_answer_an_array_and_an_image_only_its_size() {
    let bridge = Bridge::new(Fake::default(), 10);
    let answer = parsed(&bridge.call("screenshot", "{}").await.expect("an answer"));
    assert_eq!(answer[0], json!({ "views": 2 }));
    assert_eq!(answer[1]["type"], "image");
    assert_eq!(answer[1]["mimeType"], "image/png");
    assert_eq!(answer[1]["bytes"], 3);
    assert!(answer[1].get("data").is_none());
    let note = parsed(&bridge.call("note", "{}").await.expect("an answer"));
    assert_eq!(note, json!("plain words"));
}

#[tokio::test]
async fn every_call_counts_as_a_request_of_the_server() {
    let (_, before) = crate::activity::SERVER.since_last(SystemTime::now());
    let bridge = Bridge::new(Fake::default(), 10);
    bridge.call("views_list", "{}").await.expect("an answer");
    bridge.call("views_list", "{}").await.expect("an answer");
    let (_, after) = crate::activity::SERVER.since_last(SystemTime::now());
    assert!(after >= before + 2, "{before} -> {after}");
}

#[tokio::test]
async fn a_router_error_is_named_by_its_json_rpc_code_not_its_words() {
    let bridge = Bridge::new(Fake::default(), 10);
    let typed = bridge
        .call("typed", "{}")
        .await
        .expect_err("invalid params");
    assert_eq!(parsed(&typed)["code"], "args.invalid");
    assert_eq!(parsed(&typed)["legacyCode"], "invalid_arguments");
    let absent = bridge
        .call("absent", "{}")
        .await
        .expect_err("method not found");
    assert_eq!(parsed(&absent)["code"], "tool.not_found");
    let broken = bridge.call("broken", "{}").await.expect_err("internal");
    assert_eq!(parsed(&broken)["code"], "tool.failed");
    assert_eq!(parsed(&broken)["legacyCode"], "tool_failed");
}

#[test]
fn a_refusal_names_the_flat_code_only_where_one_was_renamed() {
    let renamed = parsed(&super::refusal("tool.not_found", "no such tool"));
    assert_eq!(renamed["legacyCode"], "tool_not_found");
    let kept = parsed(&super::refusal("script.recursion", "no"));
    assert!(kept.get("legacyCode").is_none(), "{kept}");
}
