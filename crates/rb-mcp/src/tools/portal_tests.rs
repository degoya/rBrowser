use std::sync::Arc;

use rmcp::handler::server::wrapper::Parameters;
use serde_json::json;

use super::{PortalDisconnectParams, PortalParams};
use crate::testing::{FakeBackend, HOST_TOOL_PREFIX, answer_json, server, view};

fn host(name: &str) -> String {
    format!("{HOST_TOOL_PREFIX}{name}")
}

fn fake() -> Arc<FakeBackend> {
    Arc::new(FakeBackend::new(vec![view(1, "mobile", "http://a.test/")]))
}

#[tokio::test]
async fn every_tool_goes_to_the_host() {
    let fake = fake();
    let server = server(&fake);
    server
        .portal_stop(Parameters(PortalParams::default()))
        .await
        .expect("answer");
    server
        .portal_devices(Parameters(PortalParams::default()))
        .await
        .expect("answer");
    server
        .portal_disconnect(Parameters(PortalDisconnectParams { device: Some(3) }))
        .await
        .expect("answer");
    server
        .portal_disconnect(Parameters(PortalDisconnectParams::default()))
        .await
        .expect("answer");
    assert_eq!(
        fake.calls(),
        [
            (host("portal_stop"), json!({})),
            (host("portal_devices"), json!({})),
            (host("portal_disconnect"), json!({ "device": 3 })),
            (host("portal_disconnect"), json!({})),
        ]
    );
}

#[tokio::test]
async fn a_failed_stop_is_an_error_mcp_can_read() {
    let fake = fake();
    fake.answer(
        &host("portal_stop"),
        Err(rb_commands::CommandError::Failed(
            "the window is closed".to_owned(),
        )),
    );
    let (error, _) = answer_json(
        &server(&fake)
            .portal_stop(Parameters(PortalParams::default()))
            .await
            .expect("answer"),
    );
    assert!(error);
}

#[test]
fn no_tool_starts_the_portal() {
    let names: Vec<String> = crate::RbMcpServer::portal_router()
        .list_all()
        .into_iter()
        .map(|tool| tool.name.to_string())
        .collect();
    assert!(
        !names.iter().any(|name| name == "portal_start"),
        "{names:?}"
    );
    assert_eq!(names.len(), 3, "{names:?}");
}

#[test]
fn unknown_arguments_are_refused() {
    assert!(serde_json::from_value::<PortalParams>(json!({ "enabled": true })).is_err());
    assert!(
        serde_json::from_value::<PortalDisconnectParams>(json!({ "device": 1, "all": true }))
            .is_err()
    );
}
