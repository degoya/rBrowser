use std::sync::Arc;

use rmcp::handler::server::wrapper::Parameters;
use serde_json::json;

use super::InspectAtParams;
use crate::target::TargetArg;
use crate::testing::{FakeBackend, answer_json, server};

#[tokio::test]
async fn inspect_at_targets_one_view_with_its_point() {
    let fake = Arc::new(FakeBackend::default());
    server(&fake)
        .devtools_inspect_at(Parameters(InspectAtParams {
            target: Some(TargetArg::Name("mobile".to_owned())),
            x: 12.0,
            y: 34.5,
        }))
        .await
        .expect("answer");
    assert_eq!(
        fake.calls(),
        vec![(
            "devtools.inspect_at".to_owned(),
            json!({ "view": 1, "x": 12.0, "y": 34.5, "open": true })
        )]
    );
}

#[tokio::test]
async fn inspect_at_without_a_target_takes_the_active_view() {
    let fake = Arc::new(FakeBackend::default());
    server(&fake)
        .devtools_inspect_at(Parameters(InspectAtParams {
            target: None,
            x: 1.0,
            y: 2.0,
        }))
        .await
        .expect("answer");
    assert_eq!(
        fake.calls(),
        vec![(
            "devtools.inspect_at".to_owned(),
            json!({ "x": 1.0, "y": 2.0, "open": true })
        )]
    );
}

#[tokio::test]
async fn inspect_at_refuses_all_and_a_negative_point() {
    let fake = Arc::new(FakeBackend::default());
    let (error, answer) = answer_json(
        &server(&fake)
            .devtools_inspect_at(Parameters(InspectAtParams {
                target: Some(TargetArg::Name("all".to_owned())),
                x: 1.0,
                y: 2.0,
            }))
            .await
            .expect("answer"),
    );
    assert!(error, "{answer}");
    let (error, answer) = answer_json(
        &server(&fake)
            .devtools_inspect_at(Parameters(InspectAtParams {
                target: None,
                x: -1.0,
                y: 2.0,
            }))
            .await
            .expect("answer"),
    );
    assert!(error, "{answer}");
    assert_eq!(answer["code"], "args.invalid");
    assert!(fake.calls().is_empty());
}
