use std::sync::Arc;

use rmcp::handler::server::wrapper::Parameters;
use serde_json::json;

use super::CdpEndpointParams;
use crate::backend::McpOptions;
use crate::target::TargetArg;
use crate::testing::{FakeBackend, answer_json, server};

/// A fake whose person switched `mcp.allowEvaluate` on: the endpoint is full control of the
/// page, so it answers only then (RB-090-21).
fn evaluating() -> Arc<FakeBackend> {
    let fake = Arc::new(FakeBackend::default());
    fake.set_options(McpOptions {
        allow_evaluate: true,
        ..McpOptions::default()
    });
    fake
}

/// The endpoint's token is Runtime.evaluate, the cookies and every navigation: while
/// `mcp.allowEvaluate` is off the tool answers `evaluate.disabled` with the setting, carries no
/// token and does not even ask the host (security review RB-090-06, finding 1).
#[tokio::test]
async fn evaluate_off_the_endpoint_is_refused_without_its_token() {
    let fake = Arc::new(FakeBackend::default());
    fake.set_cdp_endpoint(Some(json!({
        "url": "http://127.0.0.1:9222",
        "headers": { "Authorization": "Bearer t0k3n" },
    })));
    for target in [None, Some(TargetArg::Name("desktop".to_owned()))] {
        let result = server(&fake)
            .cdp_endpoint(Parameters(CdpEndpointParams { target }))
            .await
            .expect("answer");
        let (error, answer) = answer_json(&result);
        assert!(error);
        assert_eq!(answer["code"], json!("evaluate.disabled"));
        assert_eq!(answer["settingsKey"], json!("mcp.allowEvaluate"));
        assert!(!answer.to_string().contains("t0k3n"), "{answer}");
    }
    assert!(fake.cdp_endpoint_views().is_empty());
}

#[tokio::test]
async fn off_the_tool_names_the_setting() {
    let fake = evaluating();
    let result = server(&fake)
        .cdp_endpoint(Parameters(CdpEndpointParams::default()))
        .await
        .expect("answer");
    let (error, answer) = answer_json(&result);
    assert!(error);
    assert_eq!(answer["code"], json!("cdp.disabled"));
    assert_eq!(answer["settingsKey"], json!("cdp.enabled"));
}

#[tokio::test]
async fn on_the_tool_answers_the_endpoint_with_its_token() {
    let fake = evaluating();
    fake.set_cdp_endpoint(Some(json!({
        "url": "http://127.0.0.1:9222",
        "headers": { "Authorization": "Bearer t0k3n" },
    })));
    let result = server(&fake)
        .cdp_endpoint(Parameters(CdpEndpointParams::default()))
        .await
        .expect("answer");
    let (error, answer) = answer_json(&result);
    assert!(!error);
    assert_eq!(answer["headers"]["Authorization"], json!("Bearer t0k3n"));
    assert_eq!(fake.cdp_endpoint_views(), vec![None]);
}

#[tokio::test]
async fn a_target_is_resolved_to_one_view() {
    let fake = evaluating();
    fake.set_cdp_endpoint(Some(json!({ "url": "http://127.0.0.1:9222" })));
    let params = CdpEndpointParams {
        target: Some(TargetArg::Name("desktop".to_owned())),
    };
    server(&fake)
        .cdp_endpoint(Parameters(params))
        .await
        .expect("answer");
    assert_eq!(fake.cdp_endpoint_views(), vec![Some(2)]);
    let all = CdpEndpointParams {
        target: Some(TargetArg::all()),
    };
    let (error, answer) = answer_json(
        &server(&fake)
            .cdp_endpoint(Parameters(all))
            .await
            .expect("answer"),
    );
    assert!(error);
    assert_eq!(answer["code"], json!("target.not_single"));
}
