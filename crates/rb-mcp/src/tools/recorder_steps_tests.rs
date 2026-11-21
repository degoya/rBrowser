use std::sync::Arc;

use rmcp::handler::server::wrapper::Parameters;
use serde_json::json;

use super::{
    RecorderStepAddParams, RecorderStepParams, RecorderStepTryParams, RecorderStepUpdateParams,
};
use crate::target::TargetArg;
use crate::testing::{FakeBackend, HOST_TOOL_PREFIX, answer_json, server};

fn tool(name: &str) -> String {
    format!("{HOST_TOOL_PREFIX}{name}")
}

#[tokio::test]
async fn a_step_is_added_by_the_host_with_the_view_of_its_point() {
    let fake = Arc::new(FakeBackend::default());
    fake.answer(
        &tool("recorder_step_add"),
        Ok(json!({ "recordingId": "search", "stepId": 9, "index": 3, "steps": 9 })),
    );
    let (error, answer) = answer_json(
        &server(&fake)
            .recorder_step_add(Parameters(RecorderStepAddParams {
                recording_id: "search".to_owned(),
                action: json!({ "type": "click" }),
                after: Some(3),
                target: Some(TargetArg::Name("desktop".to_owned())),
                x: Some(120.0),
                y: Some(48.5),
                ..RecorderStepAddParams::default()
            }))
            .await
            .expect("answer"),
    );
    assert!(!error);
    assert_eq!(answer["stepId"], json!(9));
    assert_eq!(
        fake.calls(),
        vec![(
            tool("recorder_step_add"),
            json!({ "recordingId": "search", "action": { "type": "click" }, "after": 3,
                    "view": 2, "x": 120.0, "y": 48.5 })
        )]
    );
}

#[tokio::test]
async fn a_step_is_changed_removed_and_switched_by_the_host() {
    let fake = Arc::new(FakeBackend::default());
    let server = server(&fake);
    server
        .recorder_step_update(Parameters(RecorderStepUpdateParams {
            recording_id: "search".to_owned(),
            step_id: 4,
            patch: json!({ "pauseMs": 700 }),
        }))
        .await
        .expect("answer");
    server
        .recorder_step_remove(Parameters(RecorderStepParams {
            recording_id: "search".to_owned(),
            step_id: 5,
            disabled: Some(true),
        }))
        .await
        .expect("answer");
    server
        .recorder_step_toggle(Parameters(RecorderStepParams {
            recording_id: "search".to_owned(),
            step_id: 6,
            disabled: None,
        }))
        .await
        .expect("answer");
    assert_eq!(
        fake.calls(),
        vec![
            (
                tool("recorder_step_update"),
                json!({ "recordingId": "search", "stepId": 4, "patch": { "pauseMs": 700 } })
            ),
            // Removing a step never carries more than the step.
            (
                tool("recorder_step_remove"),
                json!({ "recordingId": "search", "stepId": 5 })
            ),
            (
                tool("recorder_step_toggle"),
                json!({ "recordingId": "search", "stepId": 6 })
            ),
        ]
    );
}

#[tokio::test]
async fn a_step_is_tried_in_one_view_and_the_answer_is_masked() {
    let fake = Arc::new(FakeBackend::default());
    fake.answer(
        &tool("recorder_step_try"),
        Ok(json!({ "passed": false, "error": "Authorization: Bearer SECRET-1" })),
    );
    let (error, answer) = answer_json(
        &server(&fake)
            .recorder_step_try(Parameters(RecorderStepTryParams {
                recording_id: "search".to_owned(),
                step_id: 2,
                target: None,
            }))
            .await
            .expect("answer"),
    );
    assert!(!error);
    assert_eq!(answer["error"], json!("***"));
    assert_eq!(
        fake.calls(),
        vec![(
            tool("recorder_step_try"),
            json!({ "recordingId": "search", "stepId": 2 })
        )]
    );
    let (error, answer) = answer_json(
        &server(&fake)
            .recorder_step_try(Parameters(RecorderStepTryParams {
                recording_id: "search".to_owned(),
                step_id: 2,
                target: Some(TargetArg::all()),
            }))
            .await
            .expect("answer"),
    );
    assert!(error);
    assert_eq!(answer["code"], json!("target_not_single"));
}

#[test]
fn the_step_tools_take_no_unknown_arguments() {
    assert!(
        serde_json::from_value::<RecorderStepParams>(
            json!({ "recordingId": "s", "stepId": 1, "deleteRecording": true })
        )
        .is_err()
    );
    assert!(
        serde_json::from_value::<RecorderStepAddParams>(json!({ "recordingId": "s" })).is_err()
    );
}
