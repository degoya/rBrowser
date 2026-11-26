use std::sync::Arc;

use rmcp::handler::server::wrapper::Parameters;
use serde_json::json;

use super::{
    ContinueFrom, ExportFormat, PlaybackPace, RecorderExportParams, RecorderPlayParams,
    RecorderStartParams, RecorderStopParams,
};
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
            continue_from: None,
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
                continue_from: None,
            }))
            .await
            .expect("answer"),
    );
    assert!(error);
    assert_eq!(answer["code"], json!("target.not_single"));
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

#[tokio::test]
async fn recorder_play_is_run_by_the_host_in_the_views_of_its_target() {
    let fake = Arc::new(FakeBackend::default());
    let tool = format!("{HOST_TOOL_PREFIX}recorder_play");
    fake.answer(
        &tool,
        Ok(json!({ "flow": "login", "passed": true, "views": [],
                   "error": "Authorization: Bearer SECRET-1" })),
    );
    let server = server(&fake);
    let (error, answer) = answer_json(
        &server
            .recorder_play(Parameters(RecorderPlayParams::default()))
            .await
            .expect("answer"),
    );
    assert!(!error);
    assert_eq!(answer["passed"], json!(true));
    assert_eq!(answer["error"], json!("***"), "masked like the log");
    server
        .recorder_play(Parameters(RecorderPlayParams {
            recording_id: Some("login".to_owned()),
            target: Some(TargetArg::all()),
            pace: Some(PlaybackPace::Recorded),
            wait: Some(false),
            from_step: None,
        }))
        .await
        .expect("answer");
    server
        .recorder_play(Parameters(RecorderPlayParams {
            target: Some(TargetArg::Name("desktop".to_owned())),
            ..RecorderPlayParams::default()
        }))
        .await
        .expect("answer");
    assert_eq!(
        fake.calls(),
        vec![
            (tool.clone(), json!({})),
            (
                tool.clone(),
                json!({ "recordingId": "login", "views": [1, 2], "pace": "recorded",
                        "wait": false })
            ),
            (tool, json!({ "views": [2] })),
        ]
    );
}

#[test]
fn the_recorder_tools_take_no_unknown_arguments() {
    assert!(serde_json::from_value::<RecorderPlayParams>(json!({ "name": "login" })).is_err());
    assert!(serde_json::from_value::<RecorderStopParams>(json!({ "target": 1 })).is_err());
    assert!(serde_json::from_value::<RecorderExportParams>(json!({ "target": "all" })).is_err());
    assert!(serde_json::from_value::<RecorderExportParams>(json!({ "format": "pdf" })).is_err());
    let both: RecorderExportParams =
        serde_json::from_value(json!({ "format": "both" })).expect("both");
    assert_eq!(both.format, Some(ExportFormat::Both));
}

#[tokio::test]
async fn recorder_start_continues_a_stored_recording_from_a_step() {
    // RB-092-01: the command plays the steps up to there, then records.
    let fake = Arc::new(FakeBackend::default());
    server(&fake)
        .recorder_start(Parameters(RecorderStartParams {
            continue_from: Some(ContinueFrom {
                recording_id: "search".to_owned(),
                step_id: 3,
            }),
            ..RecorderStartParams::default()
        }))
        .await
        .expect("answer");
    assert_eq!(
        fake.calls(),
        vec![(
            "recorder.continue_from".to_owned(),
            json!({ "recordingId": "search", "stepId": 3 })
        )]
    );
}

#[tokio::test]
async fn recorder_play_plays_from_a_step() {
    let fake = Arc::new(FakeBackend::default());
    let tool = format!("{HOST_TOOL_PREFIX}recorder_play");
    server(&fake)
        .recorder_play(Parameters(RecorderPlayParams {
            recording_id: Some("search".to_owned()),
            from_step: Some(4),
            ..RecorderPlayParams::default()
        }))
        .await
        .expect("answer");
    assert_eq!(
        fake.calls(),
        vec![(tool, json!({ "recordingId": "search", "fromStep": 4 }))]
    );
}
