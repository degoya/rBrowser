use std::sync::Arc;

use rmcp::handler::server::wrapper::Parameters;
use serde_json::json;

use super::{EVALUATE_DISABLED, EvaluateParams, SnapshotParams, evaluated, trim_tree};
use crate::backend::McpOptions;
use crate::target::TargetArg;
use crate::testing::{FakeBackend, answer_json, server};

fn evaluate(expression: &str, target: Option<TargetArg>) -> EvaluateParams {
    EvaluateParams {
        expression: expression.to_owned(),
        target,
        await_promise: None,
        timeout_ms: None,
    }
}

fn allow(fake: &FakeBackend, max_answer_bytes: usize) {
    fake.set_options(McpOptions {
        allow_evaluate: true,
        max_answer_bytes,
        ..McpOptions::default()
    });
}

#[tokio::test]
async fn evaluate_is_off_by_default_and_points_at_its_setting() {
    let fake = Arc::new(FakeBackend::default());
    let (error, answer) = answer_json(
        &server(&fake)
            .evaluate(Parameters(evaluate("1 + 1", None)))
            .await
            .expect("answer"),
    );
    assert!(error);
    assert_eq!(answer["code"], EVALUATE_DISABLED);
    assert_eq!(answer["settingsKey"], "mcp.allowEvaluate");
    assert!(
        answer["error"]
            .as_str()
            .is_some_and(|text| text.contains("rbrowser://settings/"))
    );
    assert!(fake.cdp_calls().is_empty());
}

#[tokio::test]
async fn evaluate_runs_in_each_targeted_view_by_value() {
    let fake = Arc::new(FakeBackend::default());
    allow(&fake, 10_000);
    fake.answer_cdp(
        "Runtime.evaluate",
        json!({ "result": { "type": "number", "value": 2 } }),
    );
    let (error, answer) = answer_json(
        &server(&fake)
            .evaluate(Parameters(evaluate(
                "1 + 1",
                Some(TargetArg::Name("desktop".to_owned())),
            )))
            .await
            .expect("answer"),
    );
    assert!(!error, "{answer}");
    assert_eq!(
        answer,
        json!([{ "type": "number", "value": 2, "view": 2, "breakpoint": "desktop" }])
    );
    let calls = fake.cdp_calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].1, "Runtime.evaluate");
    assert_eq!(calls[0].2["returnByValue"], true);
    assert_eq!(calls[0].2["timeout"], 10_000);
}

#[tokio::test]
async fn evaluate_answers_are_masked_and_cut() {
    let fake = Arc::new(FakeBackend::default());
    allow(&fake, 10_000);
    fake.answer_cdp(
        "Runtime.evaluate",
        json!({ "result": { "type": "string", "value": "token SECRET-123" } }),
    );
    let (_, answer) = answer_json(
        &server(&fake)
            .evaluate(Parameters(evaluate("document.cookie", None)))
            .await
            .expect("answer"),
    );
    assert_eq!(answer[0]["value"], "***");
    allow(&fake, 40);
    fake.answer_cdp(
        "Runtime.evaluate",
        json!({ "result": { "type": "string", "value": "x".repeat(500) } }),
    );
    let (_, answer) = answer_json(
        &server(&fake)
            .evaluate(Parameters(evaluate("big", None)))
            .await
            .expect("answer"),
    );
    assert_eq!(answer[0]["truncated"], true);
}

#[test]
fn an_exception_is_an_error_entry() {
    let answer = json!({ "result": {}, "exceptionDetails": { "text": "Uncaught",
        "exception": { "description": "ReferenceError: x is not defined" } } });
    assert_eq!(
        evaluated(&answer),
        json!({ "error": "ReferenceError: x is not defined" })
    );
    let undefined = json!({ "result": { "type": "undefined" } });
    assert_eq!(
        evaluated(&undefined),
        json!({ "type": "undefined", "value": null })
    );
}

#[test]
fn the_tree_drops_ignored_nodes_and_reparents() {
    let tree = json!({ "nodes": [
        { "nodeId": "1", "role": { "value": "RootWebArea" }, "name": { "value": "Shop" } },
        { "nodeId": "2", "parentId": "1", "ignored": true, "role": { "value": "generic" } },
        { "nodeId": "3", "parentId": "2", "role": { "value": "button" }, "name": { "value": "Buy" } },
        { "nodeId": "4", "parentId": "3", "role": { "value": "InlineTextBox" } },
    ] });
    let trimmed = trim_tree(&tree, 10);
    assert_eq!(
        trimmed["nodes"],
        json!([
            { "id": "1", "role": "RootWebArea", "name": "Shop" },
            { "id": "3", "role": "button", "name": "Buy", "parent": "1" },
        ])
    );
    assert_eq!(trimmed["truncated"], false);
    let one = trim_tree(&tree, 1);
    assert_eq!(one["truncated"], true);
    assert_eq!(one["total"], 2);
}

#[tokio::test]
async fn snapshot_asks_every_view() {
    let fake = Arc::new(FakeBackend::default());
    fake.answer_cdp(
        "Accessibility.getFullAXTree",
        json!({ "nodes": [{ "nodeId": "1", "role": { "value": "RootWebArea" } }] }),
    );
    let (error, answer) = answer_json(
        &server(&fake)
            .snapshot(Parameters(SnapshotParams::default()))
            .await
            .expect("answer"),
    );
    assert!(!error, "{answer}");
    assert_eq!(answer.as_array().map(Vec::len), Some(2));
    assert_eq!(answer[1]["nodes"][0]["role"], "RootWebArea");
    assert_eq!(fake.cdp_calls().len(), 2);
}
