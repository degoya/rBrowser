use std::sync::Arc;

use serde_json::json;

use crate::testing::{FakeBackend, HOST_TOOL_PREFIX, answer_json, server, view};

fn host(name: &str) -> String {
    format!("{HOST_TOOL_PREFIX}{name}")
}

/// Each tool goes to the host without arguments; the answer is the host's.
#[tokio::test]
async fn the_status_and_the_check_are_the_hosts_answers() {
    let fake = Arc::new(FakeBackend::new(vec![view(1, "mobile", "https://a.test/")]));
    fake.answer(
        &host("update_status"),
        Ok(json!({ "installed": "0.9.0-beta.2", "offered": null, "state": "idle" })),
    );
    fake.answer(
        &host("update_check"),
        Ok(json!({ "installed": "0.9.0-beta.2", "offered": "0.9.0-beta.3", "state": "available" })),
    );
    let server = server(&fake);
    let (error, status) = answer_json(&server.update_status().await.expect("answer"));
    assert!(!error, "{status}");
    assert_eq!(status["state"], "idle");
    let (error, checked) = answer_json(&server.update_check().await.expect("answer"));
    assert!(!error, "{checked}");
    assert_eq!(checked["offered"], "0.9.0-beta.3");
    assert_eq!(
        fake.calls(),
        [
            (host("update_status"), json!({})),
            (host("update_check"), json!({}))
        ]
    );
}

/// The install is the host's request to the person; the tool passes nothing that could stand
/// for their click (RB-090-14).
#[tokio::test]
async fn the_install_only_asks_the_person() {
    let fake = Arc::new(FakeBackend::new(vec![view(1, "mobile", "https://a.test/")]));
    fake.answer(
        &host("update_install"),
        Ok(json!({ "asked": true, "installed": false, "offered": "0.9.0-beta.3" })),
    );
    let (error, answer) = answer_json(&server(&fake).update_install().await.expect("answer"));
    assert!(!error, "{answer}");
    assert_eq!(answer["installed"], false);
    assert_eq!(fake.calls(), [(host("update_install"), json!({}))]);
}
