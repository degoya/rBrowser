use std::sync::Arc;

use rmcp::handler::server::wrapper::Parameters;
use serde_json::json;

use super::*;
use crate::testing::{FakeBackend, HOST_TOOL_PREFIX, SECRET_MARK, answer_json, server};

fn host_id(tool: &str) -> String {
    format!("{HOST_TOOL_PREFIX}{tool}")
}

#[tokio::test]
async fn the_breakpoints_list_reaches_the_host_and_its_answer_is_masked() {
    let fake = Arc::new(FakeBackend::default());
    fake.answer(
        &host_id(BREAKPOINTS_TOOL),
        Ok(json!({ "enabled": true, "breakpoints": [], "held": [{ "id": "h1", "url": format!("https://a.test/?t={SECRET_MARK}") }] })),
    );
    let (error, answer) = answer_json(
        &server(&fake)
            .network_breakpoints(Parameters(BreakpointsParams::default()))
            .await
            .expect("answer"),
    );
    assert!(!error, "{answer}");
    assert_eq!(answer["held"][0]["id"], "h1");
    assert_eq!(answer["held"][0]["url"], "***");
    assert_eq!(fake.calls(), [(host_id(BREAKPOINTS_TOOL), json!({}))]);
}

#[tokio::test]
async fn an_add_sends_the_breakpoint_fields() {
    let fake = Arc::new(FakeBackend::default());
    fake.answer(&host_id(BREAKPOINTS_TOOL), Ok(json!({ "id": "bp-1" })));
    server(&fake)
        .network_breakpoints(Parameters(BreakpointsParams {
            action: Some(BreakpointAction::Add),
            url_pattern: Some("*/api/*".to_owned()),
            stage: Some("response".to_owned()),
            views: Some(vec![1, 2]),
            ..BreakpointsParams::default()
        }))
        .await
        .expect("answer");
    assert_eq!(
        fake.calls(),
        [(
            host_id(BREAKPOINTS_TOOL),
            json!({ "action": "add", "urlPattern": "*/api/*", "stage": "response", "views": [1, 2] })
        )]
    );
}

#[tokio::test]
async fn continue_abort_compose_and_repeat_forward_their_arguments() {
    let fake = Arc::new(FakeBackend::default());
    for tool in [CONTINUE_TOOL, ABORT_TOOL, COMPOSE_TOOL, REPEAT_TOOL] {
        fake.answer(&host_id(tool), Ok(json!({ "ok": true })));
    }
    let server = server(&fake);
    server
        .network_continue(Parameters(ContinueParams {
            id: "h1".to_owned(),
            all_views: Some(true),
            edits: Some(EditsParams {
                status: Some(500),
                ..EditsParams::default()
            }),
        }))
        .await
        .expect("answer");
    server
        .network_abort(Parameters(AbortParams {
            id: "h2".to_owned(),
            all_views: None,
        }))
        .await
        .expect("answer");
    server
        .network_compose(Parameters(ComposeParams {
            request: RequestParams {
                method: None,
                url: "https://a.test/".to_owned(),
                headers: Some(vec![("x-a".to_owned(), "1".to_owned())]),
                body: None,
                body_base64: None,
            },
            view: Some(1),
            times: Some(2),
        }))
        .await
        .expect("answer");
    server
        .network_repeat(Parameters(RepeatParams {
            view: 1,
            request_id: "1.7".to_owned(),
            edits: None,
            times: None,
        }))
        .await
        .expect("answer");
    assert_eq!(
        fake.calls(),
        [
            (
                host_id(CONTINUE_TOOL),
                json!({ "id": "h1", "allViews": true, "edits": { "status": 500 } })
            ),
            (host_id(ABORT_TOOL), json!({ "id": "h2" })),
            (
                host_id(COMPOSE_TOOL),
                json!({ "request": { "url": "https://a.test/", "headers": [["x-a", "1"]] }, "view": 1, "times": 2 })
            ),
            (
                host_id(REPEAT_TOOL),
                json!({ "view": 1, "requestId": "1.7" })
            ),
        ]
    );
}

#[tokio::test]
async fn a_refusal_of_the_host_comes_back_with_its_code_and_setting() {
    let fake = Arc::new(FakeBackend::default());
    fake.answer(
        &host_id(COMPOSE_TOOL),
        Err(rb_commands::CommandError::Failed("off".to_owned())),
    );
    let (error, answer) = answer_json(
        &server(&fake)
            .network_compose(Parameters(ComposeParams {
                request: RequestParams {
                    method: None,
                    url: "https://a.test/".to_owned(),
                    headers: None,
                    body: None,
                    body_base64: None,
                },
                view: None,
                times: None,
            }))
            .await
            .expect("answer"),
    );
    assert!(error, "{answer}");
}

#[test]
fn unknown_arguments_are_refused() {
    assert!(serde_json::from_value::<BreakpointsParams>(json!({ "target": "all" })).is_err());
    assert!(
        serde_json::from_value::<ContinueParams>(json!({ "id": "h1", "edits": { "cookie": "x" } }))
            .is_err()
    );
    assert!(serde_json::from_value::<ComposeParams>(json!({ "request": {} })).is_err());
    assert_eq!(
        breakpoint_fields(&BreakpointsParams {
            id: Some("bp-1".to_owned()),
            enabled: Some(false),
            ..BreakpointsParams::default()
        }),
        json!({ "id": "bp-1", "enabled": false })
    );
}
