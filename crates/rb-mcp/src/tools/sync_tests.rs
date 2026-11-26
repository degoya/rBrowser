use std::sync::Arc;

use rmcp::handler::server::wrapper::Parameters;
use serde_json::json;

use super::{BusSendParams, ClickParams, Modifier, SyncAction, SyncSetParams, TypeParams};
use crate::target::TargetArg;
use crate::testing::{FakeBackend, HOST_TOOL_PREFIX, answer_json, server};

fn host(name: &str) -> String {
    format!("{HOST_TOOL_PREFIX}{name}")
}

#[tokio::test]
async fn click_hands_the_host_its_arguments_and_the_view() {
    let fake = Arc::new(FakeBackend::default());
    fake.answer(
        &host("click"),
        Ok(json!({ "views": [{ "view": 2, "status": "done" }] })),
    );
    let params = ClickParams {
        target: Some(TargetArg::Id(2)),
        selector: "#buy".to_owned(),
        click_count: Some(2),
        modifiers: vec![Modifier::Shift],
        ..ClickParams::default()
    };
    let (error, answer) = answer_json(
        &server(&fake)
            .click(Parameters(params))
            .await
            .expect("answer"),
    );
    assert!(!error, "{answer}");
    assert_eq!(answer["views"][0]["status"], "done");
    assert_eq!(
        fake.calls(),
        vec![(
            host("click"),
            json!({ "selector": "#buy", "clickCount": 2, "modifiers": ["Shift"], "target": 2 })
        )]
    );
}

#[tokio::test]
async fn type_is_the_tool_type_and_its_debug_hides_the_text() {
    let fake = Arc::new(FakeBackend::default());
    let params = TypeParams {
        target: None,
        selector: None,
        text: "hunter2".to_owned(),
    };
    assert!(!format!("{params:?}").contains("hunter2"));
    server(&fake)
        .type_text(Parameters(params))
        .await
        .expect("answer");
    assert_eq!(
        fake.calls(),
        vec![(host("type"), json!({ "text": "hunter2" }))]
    );
    assert!(
        crate::RbMcpServer::tool_names()
            .iter()
            .any(|name| name == "type")
    );
}

#[tokio::test]
async fn bus_send_goes_to_every_view_by_default() {
    let fake = Arc::new(FakeBackend::default());
    let params = BusSendParams {
        target: None,
        kind: "canvas:pan".to_owned(),
        payload: json!({ "x": 1 }),
    };
    server(&fake)
        .bus_send(Parameters(params))
        .await
        .expect("answer");
    assert_eq!(
        fake.calls(),
        vec![(
            host("bus_send"),
            json!({ "type": "canvas:pan", "payload": { "x": 1 }, "target": "all" })
        )]
    );
}

#[tokio::test]
async fn sync_set_answers_the_state_and_a_refusal_as_an_error() {
    let fake = Arc::new(FakeBackend::default());
    fake.answer(
        &host("sync_set"),
        Err(rb_commands::CommandError::InvalidArgs(
            "`set` needs `enabled`".to_owned(),
        )),
    );
    let params = SyncSetParams {
        action: Some(SyncAction::Set),
        ..SyncSetParams::default()
    };
    let (error, answer) = answer_json(
        &server(&fake)
            .sync_set(Parameters(params))
            .await
            .expect("answer"),
    );
    assert!(error);
    assert_eq!(answer["code"], "args.invalid");
    assert_eq!(
        fake.calls(),
        vec![(host("sync_set"), json!({ "action": "set" }))]
    );
}
