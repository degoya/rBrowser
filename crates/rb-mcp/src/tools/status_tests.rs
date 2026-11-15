use std::sync::Arc;

use rmcp::handler::server::wrapper::Parameters;
use serde_json::json;

use super::{STATUS_MESSAGES_TOOL, StatusLevel, StatusMessagesParams};
use crate::testing::{FakeBackend, HOST_TOOL_PREFIX, SECRET_MARK, answer_json, server};

fn host_id() -> String {
    format!("{HOST_TOOL_PREFIX}{STATUS_MESSAGES_TOOL}")
}

#[tokio::test]
async fn the_host_answers_and_a_secret_is_masked() {
    let fake = Arc::new(FakeBackend::default());
    fake.answer(
        &host_id(),
        Ok(json!({
            "messages": [{ "id": 2, "text": format!("CDP endpoint: http://127.0.0.1:9/{SECRET_MARK}") }],
            "kept": 2
        })),
    );
    let (error, answer) = answer_json(
        &server(&fake)
            .status_messages(Parameters(StatusMessagesParams::default()))
            .await
            .expect("answer"),
    );
    assert!(!error, "{answer}");
    assert_eq!(answer["messages"][0]["id"], 2);
    assert_eq!(answer["messages"][0]["text"], "***");
    assert_eq!(fake.calls(), [(host_id(), json!({}))]);
}

#[tokio::test]
async fn limit_and_level_go_to_the_host() {
    let fake = Arc::new(FakeBackend::default());
    fake.answer(&host_id(), Ok(json!({ "messages": [], "kept": 0 })));
    server(&fake)
        .status_messages(Parameters(StatusMessagesParams {
            limit: Some(5),
            level: Some(StatusLevel::Error),
        }))
        .await
        .expect("answer");
    assert_eq!(
        fake.calls(),
        [(host_id(), json!({ "limit": 5, "level": "error" }))]
    );
}

#[test]
fn unknown_arguments_and_levels_are_refused() {
    assert!(serde_json::from_value::<StatusMessagesParams>(json!({ "target": "all" })).is_err());
    assert!(serde_json::from_value::<StatusMessagesParams>(json!({ "level": "fatal" })).is_err());
}
