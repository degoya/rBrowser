use std::sync::Arc;

use serde_json::json;

use crate::testing::{FakeBackend, HOST_TOOL_PREFIX, answer_json, server};

fn host(name: &str) -> String {
    format!("{HOST_TOOL_PREFIX}{name}")
}

#[tokio::test]
async fn the_simulators_and_devices_are_the_hosts_answer() {
    let fake = Arc::new(FakeBackend::new(Vec::new()));
    fake.answer(
        &host("ios_devices"),
        Ok(json!({
            "supported": true,
            "xcode": true,
            "simulators": [ { "udid": "8B1E5A9C-2F3D-4C11-9E77-0A1B2C3D4E5F", "name": "iPhone 16" } ],
            "devices": [],
        })),
    );
    let (error, answer) = answer_json(&server(&fake).ios_devices().await.expect("answer"));
    assert!(!error, "{answer}");
    assert_eq!(answer["simulators"][0]["name"], "iPhone 16");
    assert_eq!(fake.calls(), [(host("ios_devices"), json!({}))]);
}
