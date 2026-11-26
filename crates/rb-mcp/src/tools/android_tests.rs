use std::sync::Arc;

use rmcp::handler::server::wrapper::Parameters;
use serde_json::json;

use super::ViewAddAndroidParams;
use crate::testing::{FakeBackend, HOST_TOOL_PREFIX, answer_json, server};

fn host(name: &str) -> String {
    format!("{HOST_TOOL_PREFIX}{name}")
}

#[tokio::test]
async fn the_devices_are_the_hosts_answer() {
    let fake = Arc::new(FakeBackend::new(Vec::new()));
    fake.answer(
        &host("android_devices"),
        Ok(json!({ "enabled": true, "devices": [ { "serial": "S1", "ready": true } ] })),
    );
    let (error, answer) = answer_json(
        &server(&fake)
            .android_devices(crate::tools::no_args())
            .await
            .expect("answer"),
    );
    assert!(!error, "{answer}");
    assert_eq!(answer["devices"][0]["serial"], "S1");
    assert_eq!(fake.calls(), [(host("android_devices"), json!({}))]);
}

#[tokio::test]
async fn a_device_is_added_by_its_serial_or_as_the_one_ready_device() {
    let fake = Arc::new(FakeBackend::new(Vec::new()));
    fake.answer(
        &host("view_add_android"),
        Ok(json!({ "breakpoint": "android-S1", "added": true })),
    );
    let server = server(&fake);
    let (error, answer) = answer_json(
        &server
            .view_add_android(Parameters(ViewAddAndroidParams {
                serial: Some("S1".to_owned()),
            }))
            .await
            .expect("answer"),
    );
    assert!(!error, "{answer}");
    assert_eq!(answer["breakpoint"], "android-S1");
    server
        .view_add_android(Parameters(ViewAddAndroidParams::default()))
        .await
        .expect("answer");
    assert_eq!(
        fake.calls(),
        [
            (host("view_add_android"), json!({ "serial": "S1" })),
            (host("view_add_android"), json!({})),
        ]
    );
}
