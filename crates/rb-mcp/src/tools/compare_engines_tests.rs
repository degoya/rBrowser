use std::sync::Arc;

use rmcp::handler::server::wrapper::Parameters;
use serde_json::json;

use super::CompareEnginesParams;
use crate::target::TargetArg;
use crate::testing::{FakeBackend, HOST_TOOL_PREFIX, answer_json, server};

#[tokio::test]
async fn the_target_becomes_one_view_and_the_answer_is_the_hosts() {
    let fake = Arc::new(FakeBackend::default());
    fake.answer(
        &format!("{HOST_TOOL_PREFIX}compare_engines"),
        Ok(json!({ "runId": "2026-11-20_10-00-00", "differing": 1 })),
    );
    let (error, answer) = answer_json(
        &server(&fake)
            .compare_engines(Parameters(CompareEnginesParams {
                target: Some(TargetArg::Id(1)),
                breakpoint: Some("mobile".to_owned()),
                engines: vec!["chromium".to_owned(), "firefox".to_owned()],
                ..CompareEnginesParams::default()
            }))
            .await
            .expect("answer"),
    );
    assert!(!error, "{answer}");
    assert_eq!(answer["differing"], 1);
    assert_eq!(
        fake.calls(),
        [(
            format!("{HOST_TOOL_PREFIX}compare_engines"),
            json!({ "breakpoint": "mobile", "engines": ["chromium", "firefox"], "view": 1 })
        )]
    );
}

#[tokio::test]
async fn bounds_are_held_before_the_host() {
    let fake = Arc::new(FakeBackend::default());
    let server = server(&fake);
    for params in [
        CompareEnginesParams {
            threshold: Some(2.0),
            ..CompareEnginesParams::default()
        },
        CompareEnginesParams {
            max_diff_percent: Some(-1.0),
            ..CompareEnginesParams::default()
        },
    ] {
        let (error, _) = answer_json(
            &server
                .compare_engines(Parameters(params))
                .await
                .expect("answer"),
        );
        assert!(error);
    }
    assert!(fake.calls().is_empty());
}

#[test]
fn unknown_arguments_are_refused() {
    assert!(serde_json::from_value::<CompareEnginesParams>(json!({ "open": true })).is_err());
    assert!(serde_json::from_value::<CompareEnginesParams>(json!({ "view": 1 })).is_err());
}
