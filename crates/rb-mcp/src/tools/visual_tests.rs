use std::sync::Arc;

use rmcp::handler::server::wrapper::Parameters;
use serde_json::json;

use super::{BaselineUpdateParams, CompareParams};
use crate::target::TargetArg;
use crate::testing::{FakeBackend, HOST_TOOL_PREFIX, answer_json, server, view};

#[tokio::test]
async fn visual_compare_is_run_by_the_host_with_its_arguments() {
    let fake = Arc::new(FakeBackend::default());
    let server = server(&fake);
    server
        .visual_compare(Parameters(CompareParams::default()))
        .await
        .expect("answer");
    server
        .visual_compare(Parameters(CompareParams {
            target: Some(TargetArg::Id(2)),
            threshold: Some(0.2),
            ..CompareParams::default()
        }))
        .await
        .expect("answer");
    let tool = format!("{HOST_TOOL_PREFIX}visual_compare");
    assert_eq!(
        fake.calls(),
        vec![
            (tool.clone(), json!({})),
            (tool, json!({ "threshold": 0.2, "target": 2 })),
        ]
    );
}

#[tokio::test]
async fn a_breakpoint_shown_twice_compares_both_views_and_adds_up_the_regressions() {
    let fake = Arc::new(FakeBackend::new(vec![
        view(1, "mobile", "about:blank"),
        view(3, "mobile", "about:blank"),
    ]));
    fake.answer(
        &format!("{HOST_TOOL_PREFIX}visual_compare"),
        Ok(json!({ "runId": "r", "comparisons": [{ "view": 1 }], "regressions": 1, "unreliable": 0 })),
    );
    let (error, answer) = answer_json(
        &server(&fake)
            .visual_compare(Parameters(CompareParams {
                target: Some(TargetArg::Name("mobile".to_owned())),
                threshold: None,
                ..CompareParams::default()
            }))
            .await
            .expect("answer"),
    );
    assert!(!error);
    assert_eq!(answer["regressions"], json!(2));
    assert_eq!(answer["comparisons"].as_array().map(Vec::len), Some(2));
}

#[tokio::test]
async fn visual_baseline_update_is_run_by_the_host() {
    let fake = Arc::new(FakeBackend::default());
    server(&fake)
        .visual_baseline_update(Parameters(BaselineUpdateParams {
            target: Some(TargetArg::all()),
            ..BaselineUpdateParams::default()
        }))
        .await
        .expect("answer");
    assert_eq!(
        fake.calls(),
        vec![(
            format!("{HOST_TOOL_PREFIX}visual_baseline_update"),
            json!({ "target": "all" })
        )]
    );
}

#[tokio::test]
async fn a_threshold_outside_zero_to_one_is_refused_before_the_host_runs() {
    let fake = Arc::new(FakeBackend::default());
    let (error, answer) = answer_json(
        &server(&fake)
            .visual_compare(Parameters(CompareParams {
                target: None,
                threshold: Some(1.5),
                ..CompareParams::default()
            }))
            .await
            .expect("answer"),
    );
    assert!(error);
    assert_eq!(answer["code"], json!("invalid_arguments"));
    assert!(fake.calls().is_empty());
}
