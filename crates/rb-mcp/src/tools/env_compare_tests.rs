use std::sync::Arc;

use rmcp::handler::server::wrapper::Parameters;
use serde_json::json;

use super::{EnvCompareLiveParams, EnvCompareParams, EnvCompareResultParams};
use crate::target::TargetArg;
use crate::testing::{FakeBackend, HOST_TOOL_PREFIX, answer_json, server};

#[tokio::test]
async fn env_compare_resolves_its_target_into_views_and_answers_the_run() {
    let fake = Arc::new(FakeBackend::default());
    fake.answer(
        &format!("{HOST_TOOL_PREFIX}env_compare"),
        Ok(json!({ "runId": "2026-11-12_10-00-00", "folder": "f", "shots": 8, "diffs": 4 })),
    );
    let (error, answer) = answer_json(
        &server(&fake)
            .env_compare(Parameters(EnvCompareParams {
                target: Some(TargetArg::Name("desktop".to_owned())),
                environments: vec!["local".to_owned(), "staging".to_owned()],
                paths: vec!["/about".to_owned()],
                ..EnvCompareParams::default()
            }))
            .await
            .expect("answer"),
    );
    assert!(!error, "{answer}");
    assert_eq!(answer["runId"], "2026-11-12_10-00-00");
    assert_eq!(
        fake.calls(),
        [(
            format!("{HOST_TOOL_PREFIX}env_compare"),
            json!({ "environments": ["local", "staging"], "paths": ["/about"], "views": [2] })
        )]
    );
}

#[tokio::test]
async fn result_and_live_reach_the_host_and_a_bad_threshold_does_not() {
    let fake = Arc::new(FakeBackend::default());
    let server = server(&fake);
    server
        .env_compare_result(Parameters(EnvCompareResultParams {
            run_id: Some("r1".to_owned()),
        }))
        .await
        .expect("answer");
    server
        .env_compare_live(Parameters(EnvCompareLiveParams {
            target: Some(TargetArg::Id(1)),
            enabled: Some(true),
            ..EnvCompareLiveParams::default()
        }))
        .await
        .expect("answer");
    let (error, _) = answer_json(
        &server
            .env_compare(Parameters(EnvCompareParams {
                threshold: Some(2.0),
                ..EnvCompareParams::default()
            }))
            .await
            .expect("answer"),
    );
    assert!(error);
    assert_eq!(
        fake.calls(),
        [
            (
                format!("{HOST_TOOL_PREFIX}env_compare_result"),
                json!({ "runId": "r1" })
            ),
            (
                format!("{HOST_TOOL_PREFIX}env_compare_live"),
                json!({ "enabled": true, "view": 1 })
            ),
        ]
    );
}

/// Charge 9 review M2: a run can be cancelled over MCP.
#[tokio::test]
async fn cancel_reaches_the_host() {
    let fake = Arc::new(FakeBackend::default());
    let server = server(&fake);
    server
        .env_compare_cancel(Parameters(EnvCompareResultParams::default()))
        .await
        .expect("answer");
    server
        .env_compare_cancel(Parameters(EnvCompareResultParams {
            run_id: Some("r1".to_owned()),
        }))
        .await
        .expect("answer");
    assert_eq!(
        fake.calls(),
        [
            (format!("{HOST_TOOL_PREFIX}env_compare_cancel"), json!({})),
            (
                format!("{HOST_TOOL_PREFIX}env_compare_cancel"),
                json!({ "runId": "r1" })
            ),
        ]
    );
}

#[test]
fn unknown_arguments_are_refused() {
    assert!(serde_json::from_value::<EnvCompareParams>(json!({ "url": "x" })).is_err());
    assert!(serde_json::from_value::<EnvCompareResultParams>(json!({ "id": "r1" })).is_err());
    assert!(serde_json::from_value::<EnvCompareLiveParams>(json!({ "view": 1 })).is_err());
}
