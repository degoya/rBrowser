use std::sync::Arc;

use rmcp::handler::server::wrapper::Parameters;
use serde_json::{Value, json};

use super::{AnimationsParams, ClockParams};
use crate::target::TargetArg;
use crate::testing::{FakeBackend, answer_json, server, view};

fn two_views() -> Arc<FakeBackend> {
    Arc::new(FakeBackend::new(vec![
        view(1, "mobile", "https://a.test/"),
        view(2, "desktop", "https://a.test/"),
    ]))
}

async fn clock(fake: &Arc<FakeBackend>, params: ClockParams) -> (bool, Value) {
    answer_json(
        &server(fake)
            .clock_set(Parameters(params))
            .await
            .expect("answer"),
    )
}

async fn animations(fake: &Arc<FakeBackend>, params: AnimationsParams) -> (bool, Value) {
    answer_json(
        &server(fake)
            .animations_set(Parameters(params))
            .await
            .expect("answer"),
    )
}

/// One clock for every view: one call of the command, no target.
#[tokio::test]
async fn a_clock_runs_the_command_once_for_every_view() {
    let fake = two_views();
    let (error, _) = clock(
        &fake,
        ClockParams {
            time: Some("2026-12-24T10:00".to_owned()),
            speed: Some(0.0),
            ..ClockParams::default()
        },
    )
    .await;
    assert!(!error);
    clock(
        &fake,
        ClockParams {
            reset: Some(true),
            ..ClockParams::default()
        },
    )
    .await;
    assert_eq!(
        fake.calls(),
        vec![
            (
                "emulation.clock".to_owned(),
                json!({ "time": "2026-12-24T10:00", "speed": 0.0 })
            ),
            ("emulation.clock".to_owned(), json!({ "reset": true })),
        ]
    );
}

/// The shortcut's toggle is no MCP call: an agent says what it wants.
#[tokio::test]
async fn a_clock_call_without_time_speed_or_reset_is_refused() {
    let fake = two_views();
    let (error, answer) = clock(&fake, ClockParams::default()).await;
    assert!(error);
    assert_eq!(answer["code"], json!("invalid_arguments"));
    assert!(fake.calls().is_empty(), "nothing ran");
}

#[tokio::test]
async fn a_rate_or_a_step_runs_per_resolved_target() {
    let fake = two_views();
    animations(
        &fake,
        AnimationsParams {
            target: Some(TargetArg::all()),
            rate: Some(0.0),
            ..AnimationsParams::default()
        },
    )
    .await;
    animations(
        &fake,
        AnimationsParams {
            target: Some(TargetArg::Name("desktop".to_owned())),
            step_ms: Some(100),
            ..AnimationsParams::default()
        },
    )
    .await;
    animations(
        &fake,
        AnimationsParams {
            rate: Some(1.0),
            ..AnimationsParams::default()
        },
    )
    .await;
    assert_eq!(
        fake.calls(),
        vec![
            (
                "overlays.animations".to_owned(),
                json!({ "rate": 0.0, "target": "all" })
            ),
            (
                "overlays.animations".to_owned(),
                json!({ "step": 100, "target": 2 })
            ),
            ("overlays.animations".to_owned(), json!({ "rate": 1.0 })),
        ]
    );
}

#[tokio::test]
async fn a_rate_and_a_step_together_or_neither_are_refused() {
    let fake = two_views();
    for params in [
        AnimationsParams::default(),
        AnimationsParams {
            rate: Some(0.5),
            step_ms: Some(100),
            ..AnimationsParams::default()
        },
    ] {
        let (error, answer) = animations(&fake, params).await;
        assert!(error);
        assert_eq!(answer["code"], json!("invalid_arguments"));
    }
    assert!(fake.calls().is_empty(), "nothing ran");
}
