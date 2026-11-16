use std::sync::Arc;

use rmcp::handler::server::wrapper::Parameters;
use serde_json::json;

use super::{ExportFormat, RecorderExportParams, RecorderStartParams, RecorderStopParams};
use crate::target::TargetArg;
use crate::testing::{FakeBackend, HOST_TOOL_PREFIX, answer_json, server};

#[tokio::test]
async fn recorder_start_runs_recorder_start_in_the_active_or_the_named_view() {
    let fake = Arc::new(FakeBackend::default());
    let server = server(&fake);
    server
        .recorder_start(Parameters(RecorderStartParams::default()))
        .await
        .expect("answer");
    server
        .recorder_start(Parameters(RecorderStartParams {
            target: Some(TargetArg::Name("desktop".to_owned())),
            name: Some("login".to_owned()),
        }))
        .await
        .expect("answer");
    assert_eq!(
        fake.calls(),
        vec![
            ("recorder.start".to_owned(), json!({})),
            (
                "recorder.start".to_owned(),
                json!({ "name": "login", "target": 2 })
            ),
        ]
    );
}

#[tokio::test]
async fn recorder_start_records_one_view_not_all() {
    let fake = Arc::new(FakeBackend::default());
    let (error, answer) = answer_json(
        &server(&fake)
            .recorder_start(Parameters(RecorderStartParams {
                target: Some(TargetArg::all()),
                name: None,
            }))
            .await
            .expect("answer"),
    );
    assert!(error);
    assert_eq!(answer["code"], json!("target_not_single"));
    assert!(fake.calls().is_empty());
}

#[tokio::test]
async fn recorder_stop_stops_or_pauses() {
    let fake = Arc::new(FakeBackend::default());
    let server = server(&fake);
    server
        .recorder_stop(Parameters(RecorderStopParams::default()))
        .await
        .expect("answer");
    server
        .recorder_stop(Parameters(RecorderStopParams { pause: true }))
        .await
        .expect("answer");
    assert_eq!(
        fake.calls(),
        vec![
            ("recorder.stop".to_owned(), json!({})),
            ("recorder.pause".to_owned(), json!({})),
        ]
    );
}

#[tokio::test]
async fn recorder_export_is_run_by_the_host_and_its_answer_is_masked() {
    let fake = Arc::new(FakeBackend::default());
    let tool = format!("{HOST_TOOL_PREFIX}recorder_export");
    fake.answer(
        &tool,
        Ok(json!({
            "recordingId": "login",
            "folder": ".rbrowser/out/exports/login",
            "files": [{ "path": "a.ts", "content": "Authorization: Bearer SECRET-1" }],
        })),
    );
    let params = RecorderExportParams {
        recording_id: Some("login".to_owned()),
        format: Some(ExportFormat::Spec),
        name: None,
    };
    let (error, answer) = answer_json(
        &server(&fake)
            .recorder_export(Parameters(params))
            .await
            .expect("answer"),
    );
    assert!(!error);
    assert_eq!(answer["files"][0]["content"], json!("***"));
    assert_eq!(answer["folder"], json!(".rbrowser/out/exports/login"));
    assert_eq!(
        fake.calls(),
        vec![(tool, json!({ "recordingId": "login", "format": "spec" }))]
    );
}

#[test]
fn the_recorder_tools_take_no_unknown_arguments() {
    assert!(serde_json::from_value::<RecorderStopParams>(json!({ "target": 1 })).is_err());
    assert!(serde_json::from_value::<RecorderExportParams>(json!({ "target": "all" })).is_err());
    assert!(serde_json::from_value::<RecorderExportParams>(json!({ "format": "pdf" })).is_err());
    let both: RecorderExportParams =
        serde_json::from_value(json!({ "format": "both" })).expect("both");
    assert_eq!(both.format, Some(ExportFormat::Both));
}
