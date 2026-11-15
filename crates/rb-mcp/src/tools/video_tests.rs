use std::sync::Arc;

use rb_commands::CommandError;
use rmcp::handler::server::wrapper::Parameters;
use serde_json::json;

use super::{RecordVideoParams, VideoAction};
use crate::target::TargetArg;
use crate::testing::{FakeBackend, HOST_TOOL_PREFIX, answer_json, server, view};

fn host(name: &str) -> String {
    format!("{HOST_TOOL_PREFIX}{name}")
}

fn fake() -> Arc<FakeBackend> {
    Arc::new(FakeBackend::new(vec![
        view(1, "mobile", "https://a.test/"),
        view(2, "desktop", "https://a.test/"),
        view(3, "mobile", "https://a.test/"),
    ]))
}

/// A breakpoint id names every view showing it, and they start in one recording.
#[tokio::test]
async fn start_resolves_the_target_to_views_and_starts_them_together() {
    let fake = fake();
    fake.answer(
        &host("record_video"),
        Ok(json!({ "views": [1, 3], "folder": "f", "maxDurationSec": 300 })),
    );
    let (error, answer) = answer_json(
        &server(&fake)
            .record_video(Parameters(RecordVideoParams {
                action: VideoAction::Start,
                target: Some(TargetArg::Name("mobile".to_owned())),
            }))
            .await
            .expect("answer"),
    );
    assert!(!error, "{answer}");
    assert_eq!(answer["maxDurationSec"], 300);
    assert_eq!(
        fake.calls(),
        [(
            host("record_video"),
            json!({ "action": "start", "views": [1, 3] })
        )]
    );
}

#[tokio::test]
async fn start_without_a_target_records_every_view() {
    let fake = fake();
    server(&fake)
        .record_video(Parameters(RecordVideoParams {
            action: VideoAction::Start,
            target: None,
        }))
        .await
        .expect("answer");
    assert_eq!(
        fake.calls(),
        [(
            host("record_video"),
            json!({ "action": "start", "views": [1, 2, 3] })
        )]
    );
}

#[tokio::test]
async fn stop_and_status_go_to_the_host_and_stop_takes_no_target() {
    let fake = fake();
    let server = server(&fake);
    for action in [VideoAction::Stop, VideoAction::Status] {
        server
            .record_video(Parameters(RecordVideoParams {
                action,
                target: None,
            }))
            .await
            .expect("answer");
    }
    let (error, _) = answer_json(
        &server
            .record_video(Parameters(RecordVideoParams {
                action: VideoAction::Stop,
                target: Some(TargetArg::all()),
            }))
            .await
            .expect("answer"),
    );
    assert!(error, "a target on stop is refused");
    assert_eq!(
        fake.calls(),
        [
            (host("record_video"), json!({ "action": "stop" })),
            (host("record_video"), json!({ "action": "status" })),
        ]
    );
}

/// Without ffmpeg the host's refusal reaches the agent with its words.
#[tokio::test]
async fn a_refusal_of_the_host_is_answered_as_an_error() {
    let fake = fake();
    fake.answer(
        &host("record_video"),
        Err(CommandError::Failed(
            "rBrowser needs ffmpeg to record video".to_owned(),
        )),
    );
    let (error, answer) = answer_json(
        &server(&fake)
            .record_video(Parameters(RecordVideoParams {
                action: VideoAction::Start,
                target: None,
            }))
            .await
            .expect("answer"),
    );
    assert!(error);
    assert!(answer.to_string().contains("needs ffmpeg"), "{answer}");
}

#[test]
fn unknown_arguments_and_actions_are_refused() {
    assert!(serde_json::from_value::<RecordVideoParams>(json!({ "action": "pause" })).is_err());
    assert!(
        serde_json::from_value::<RecordVideoParams>(json!({ "action": "start", "x": 1 })).is_err()
    );
}
