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

/// RB-050-01 review: a recording the agent would never feed is refused, with the reason.
#[test]
fn a_recording_nothing_feeds_is_refused() {
    let on = SyncSwitches::default();
    assert_eq!(uncaptured(on, false, true), None);
    let off = SyncSwitches {
        enabled: false,
        ..on
    };
    assert!(uncaptured(off, false, true).is_some_and(|why| why.contains("sync is off")));
    assert!(uncaptured(on, true, true).is_some_and(|why| why.contains("excluded")));
    assert!(uncaptured(on, false, false).is_some_and(|why| why.contains("no view")));
}

/// The step kinds a recording lacks because their sync switch is off.
#[test]
fn the_kinds_not_captured_follow_the_switches_and_the_settings() {
    let on = SyncSwitches::default();
    assert!(missing_kinds(on, true, true).is_empty());
    let off = SyncSwitches {
        keyboard: false,
        hover: false,
        scroll: false,
        ..on
    };
    assert_eq!(missing_kinds(off, true, false), ["keyboard", "hover"]);
    assert_eq!(missing_kinds(off, false, true), ["keyboard", "scroll"]);
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
