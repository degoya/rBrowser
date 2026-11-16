use std::time::Duration;

use super::*;
use crate::event::{MouseButton, MouseData};
use crate::switches::tests::target;

fn request(target_view: u32) -> ReplayRequest {
    ReplayRequest {
        source: ViewId(1),
        target: ViewId(target_view),
        top: true,
        frame: None,
        event: SyncEvent::Click(MouseData {
            button: MouseButton::Left,
            detail: 1,
            modifiers: 0,
            target: target("button"),
        })
        .into(),
        block_submit: true,
    }
}

#[tokio::test]
async fn the_double_records_and_answers_per_view() {
    let replay = RecordingReplay::default();
    replay.answer(ViewId(3), ReplayOutcome::NotFound);
    assert_eq!(replay.dispatch(request(2)).await, ReplayOutcome::Done);
    assert_eq!(replay.dispatch(request(3)).await, ReplayOutcome::NotFound);
    assert_eq!(replay.calls().len(), 2);
    assert_eq!(replay.calls_for(ViewId(3))[0].target, ViewId(3));
}

#[tokio::test]
async fn a_stalled_view_never_answers() {
    let replay = RecordingReplay::default();
    replay.stall(ViewId(2));
    let answer = tokio::time::timeout(Duration::from_millis(30), replay.dispatch(request(2))).await;
    assert!(answer.is_err());
    assert_eq!(
        replay.calls_for(ViewId(2)).len(),
        1,
        "recorded all the same"
    );
}

#[tokio::test]
async fn no_replay_desyncs_nothing() {
    assert_eq!(
        NoReplay.dispatch(request(2)).await,
        ReplayOutcome::Unsupported
    );
}
