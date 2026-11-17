use std::sync::Arc;

use rb_engine::ViewId;
use rmcp::handler::server::wrapper::Parameters;
use serde_json::json;

use super::LogParams;
use crate::backend::Backend as _;
use crate::target::TargetArg;
use crate::testing::{FakeBackend, answer_json, server};

fn console(fake: &FakeBackend, view: u32, level: &str, text: &str) {
    fake.logs().record(
        ViewId(view),
        "Runtime.consoleAPICalled",
        &json!({ "type": level, "args": [{ "type": "string", "value": text }] }),
    );
}

#[tokio::test]
async fn console_logs_answers_per_view_filtered_limited_and_masked() {
    let fake = Arc::new(FakeBackend::default());
    console(&fake, 1, "log", "a");
    console(&fake, 1, "error", "b");
    console(&fake, 1, "error", "c SECRET");
    console(&fake, 2, "log", "other view");
    let params = LogParams {
        target: Some(TargetArg::Id(1)),
        level: Some("error".to_owned()),
        limit: Some(5),
        ..LogParams::default()
    };
    let (error, answer) = answer_json(
        &server(&fake)
            .console_logs(Parameters(params))
            .await
            .expect("answer"),
    );
    assert!(!error, "{answer}");
    assert_eq!(answer.as_array().map(Vec::len), Some(1));
    let texts: Vec<&str> = answer[0]["entries"]
        .as_array()
        .expect("entries")
        .iter()
        .filter_map(|entry| entry["text"].as_str())
        .collect();
    assert_eq!(texts, vec!["b", "***"]);
}

#[tokio::test]
async fn clear_empties_and_limit_keeps_the_newest() {
    let fake = Arc::new(FakeBackend::default());
    for text in ["1", "2", "3"] {
        console(&fake, 2, "log", text);
    }
    let server = server(&fake);
    let params = LogParams {
        target: Some(TargetArg::Name("desktop".to_owned())),
        limit: Some(2),
        clear: true,
        ..LogParams::default()
    };
    let (_, answer) = answer_json(
        &server
            .console_logs(Parameters(params))
            .await
            .expect("answer"),
    );
    assert_eq!(answer[0]["entries"][0]["text"], "2");
    assert_eq!(answer[0]["entries"][1]["text"], "3");
    assert!(fake.logs().console(ViewId(2), 10).is_empty());
}

#[tokio::test]
async fn network_requests_can_keep_only_failures_and_errors_list_exceptions() {
    let fake = Arc::new(FakeBackend::default());
    let store = fake.logs();
    for (id, status) in [("ok", 200), ("missing", 404)] {
        store.record(
            ViewId(1),
            "Network.requestWillBeSent",
            &json!({ "requestId": id, "request": { "url": format!("https://a.test/{id}"), "method": "GET" } }),
        );
        store.record(
            ViewId(1),
            "Network.responseReceived",
            &json!({ "requestId": id, "response": { "status": status } }),
        );
    }
    store.record(
        ViewId(1),
        "Runtime.exceptionThrown",
        &json!({ "exceptionDetails": { "text": "boom" } }),
    );
    let server = server(&fake);
    let failed = LogParams {
        target: Some(TargetArg::Id(1)),
        failed_only: true,
        ..LogParams::default()
    };
    let (_, answer) = answer_json(
        &server
            .network_requests(Parameters(failed))
            .await
            .expect("answer"),
    );
    assert_eq!(answer[0]["entries"].as_array().map(Vec::len), Some(1));
    assert_eq!(answer[0]["entries"][0]["status"], 404);
    let (_, answer) = answer_json(
        &server
            .errors(Parameters(LogParams::default()))
            .await
            .expect("answer"),
    );
    assert_eq!(answer[0]["entries"][0]["text"], "boom");
    assert_eq!(answer[1]["entries"], json!([]));
}
