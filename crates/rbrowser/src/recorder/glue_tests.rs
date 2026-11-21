use super::*;

#[test]
fn the_default_name_is_a_local_timestamp() {
    // RB-090-69: 2026-11-10 21:05:09 UTC, in UTC, CET and across midnight.
    assert_eq!(name_at(1_794_344_709_000, 0), "recording-20261110-210509");
    assert_eq!(
        name_at(1_794_344_709_000, 3600),
        "recording-20261110-220509"
    );
    assert_eq!(
        name_at(1_794_344_709_000, 3 * 3600),
        "recording-20261111-000509"
    );
    // 2026-07-20 10:00:00 UTC in CEST.
    assert_eq!(
        name_at(1_784_541_600_000, 7200),
        "recording-20260720-120000"
    );
    let now = now_ms();
    assert!(check_name(&default_name(now)).is_ok());
}

#[test]
fn the_indicator_shows_only_a_running_recording() {
    let outcome = |state| Outcome {
        state,
        name: Some("login".into()),
        steps: 4,
        path: None,
        not_captured: Vec::new(),
    };
    assert_eq!(status_of(&outcome(RecorderState::Idle)), None);
    assert_eq!(
        status_of(&outcome(RecorderState::Paused)),
        Some(RecorderStatus {
            phase: RecorderPhase::Paused,
            name: "login".into(),
            steps: 4,
        })
    );
}

#[test]
fn no_recording_takes_steps_while_a_flow_runs() {
    // The flow's navigations would become the recording's steps (RB-090-71).
    let refused = not_while_flow(Some("checkout"));
    let Err(RecorderFailure::FlowRunning(flow)) = &refused else {
        panic!("refused: {refused:?}");
    };
    assert_eq!(flow, "checkout");
    let message = RecorderFailure::FlowRunning("checkout".to_owned()).to_string();
    assert!(message.contains("flows.cancel"));
    assert!(message.contains(FLOW_RUNNING));
    assert!(not_while_flow(None).is_ok());
}
