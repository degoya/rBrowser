use std::sync::atomic::{AtomicUsize, Ordering};

use super::*;
use crate::desync::DesyncReason;
use crate::event::{MouseButton, MouseData, SubmitData, SyncEvent};
use crate::replay::RecordingReplay;
use crate::switches::SyncSwitches;
use crate::switches::tests::target;

const A: ViewId = ViewId(1);
const B: ViewId = ViewId(2);
const C: ViewId = ViewId(3);

fn message(seq: u64, event: SyncEvent) -> EventMessage {
    EventMessage {
        v: 1,
        top: true,
        frame: None,
        seq,
        time: 0,
        event,
    }
}

fn click(seq: u64, path: &str) -> EventMessage {
    message(
        seq,
        SyncEvent::Click(MouseData {
            button: MouseButton::Left,
            detail: 1,
            modifiers: 0,
            target: target(path),
        }),
    )
}

struct Setup {
    hub: Arc<SyncHub>,
    replay: Arc<RecordingReplay>,
    changes: Arc<AtomicUsize>,
}

fn setup(timeout: Duration) -> Setup {
    let replay = Arc::new(RecordingReplay::default());
    let changes = Arc::new(AtomicUsize::new(0));
    let counter = changes.clone();
    let hub = SyncHub::new(
        Hub::new(SyncSwitches::default()),
        replay.clone(),
        timeout,
        Handle::current(),
        Arc::new(move || {
            counter.fetch_add(1, Ordering::SeqCst);
        }),
    );
    hub.set_views(&[A, B, C]);
    hub.with(|hub| hub.set_active(Some(A)));
    Setup {
        hub,
        replay,
        changes,
    }
}

/// Lets the queues run until `done` holds; fails the test after `limit`.
async fn within(limit: Duration, mut done: impl FnMut() -> bool) {
    let started = Instant::now();
    while started.elapsed() < limit {
        if done() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("not reached within {limit:?}");
}

fn desync_of(hub: &SyncHub, view: ViewId) -> Option<DesyncReason> {
    hub.with(|hub| hub.status())
        .desynced
        .iter()
        .find(|desync| desync.view == view)
        .map(|desync| desync.reason)
}

/// K2: the mobile view lacks the desktop menu item - it shows the badge within a second, the
/// others click.
#[tokio::test(flavor = "multi_thread")]
async fn a_missing_target_desyncs_its_view_within_a_second_and_the_others_go_on() {
    let setup = setup(DEFAULT_TIMEOUT);
    setup.replay.answer(C, ReplayOutcome::NotFound);
    assert!(setup.hub.on_event(A, &click(1, "nav > a.menu")).is_replay());
    within(Duration::from_secs(1), || {
        desync_of(&setup.hub, C) == Some(DesyncReason::NotFound)
    })
    .await;
    assert_eq!(setup.replay.calls_for(B).len(), 1, "B clicked");
    assert!(
        setup.changes.load(Ordering::SeqCst) >= 1,
        "the shell hears of it"
    );
    // C is left out from now on; B goes on.
    setup.hub.on_event(A, &click(2, "main > a"));
    within(Duration::from_secs(1), || {
        setup.replay.calls_for(B).len() == 2
    })
    .await;
    assert_eq!(setup.replay.calls_for(C).len(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_hung_view_times_out_into_a_desync_and_holds_up_nobody() {
    // C's timeout is longer than the poll's limit: B's second click comes while C still hangs,
    // or - had B waited for C - not within the limit at all. No clock is read.
    let setup = setup(Duration::from_secs(2));
    setup.replay.stall(C);
    setup.hub.on_event(A, &click(1, "button"));
    setup.hub.on_event(A, &click(2, "button"));
    within(Duration::from_secs(1), || {
        setup.replay.calls_for(B).len() == 2
    })
    .await;
    assert_eq!(desync_of(&setup.hub, C), None, "B did not wait for C");
    within(Duration::from_secs(3), || {
        desync_of(&setup.hub, C) == Some(DesyncReason::Timeout)
    })
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn each_view_gets_the_events_in_capture_order() {
    let setup = setup(DEFAULT_TIMEOUT);
    for seq in 1..=20 {
        setup
            .hub
            .on_event(A, &click(seq, &format!("li:nth-of-type({seq})")));
    }
    within(Duration::from_secs(1), || {
        setup.replay.calls_for(B).len() == 20 && setup.replay.calls_for(C).len() == 20
    })
    .await;
    for view in [B, C] {
        let paths: Vec<String> = setup
            .replay
            .calls_for(view)
            .iter()
            .filter_map(|request| crate::switches::selector_of(&request.event))
            .collect();
        let expected: Vec<String> = (1..=20)
            .map(|seq| format!("li:nth-of-type({seq})"))
            .collect();
        assert_eq!(paths, expected, "{view}");
    }
}

/// K4 with the test double: the submit button's click reaches the other views with
/// `block_submit`, the submit itself only starts a follow.
#[tokio::test(flavor = "multi_thread")]
async fn a_leader_submit_blocks_the_others_submits_and_they_follow() {
    let setup = setup(DEFAULT_TIMEOUT);
    setup.hub.on_event(A, &click(1, "form > button"));
    let route = setup.hub.on_event(
        A,
        &message(
            2,
            SyncEvent::Submit(SubmitData {
                method: "post".into(),
                action: "http://localhost/form".into(),
                target: target("form"),
            }),
        ),
    );
    assert_eq!(
        route,
        Route::Follow {
            leader: A,
            followers: vec![B, C]
        }
    );
    within(Duration::from_secs(1), || setup.replay.calls().len() == 2).await;
    assert!(
        setup
            .replay
            .calls()
            .iter()
            .all(|request| request.block_submit && !matches!(*request.event, SyncEvent::Submit(_)))
    );
    let follow = setup
        .hub
        .with(|hub| hub.navigated(A, "http://localhost/thanks", Instant::now()));
    assert_eq!(follow.len(), 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn the_engine_s_replay_plugs_in_later() {
    let setup = setup(DEFAULT_TIMEOUT);
    let engine = Arc::new(RecordingReplay::default());
    setup.hub.set_replay(engine.clone());
    setup.hub.on_event(A, &click(1, "button"));
    within(Duration::from_secs(1), || engine.calls().len() == 2).await;
    assert!(setup.replay.calls().is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_view_too_far_behind_is_desynced() {
    let setup = setup(Duration::from_secs(30));
    setup.replay.stall(C);
    for seq in 0..(QUEUE_DEPTH as u64 + 2) {
        setup.hub.on_event(A, &click(seq, "button"));
    }
    assert_eq!(desync_of(&setup.hub, C), Some(DesyncReason::Timeout));
    assert!(setup.changes.load(Ordering::SeqCst) >= 1);
}

/// The skip hint in the grid gap goes when the skipped click arrives (RB-020-05).
#[tokio::test(flavor = "multi_thread")]
async fn consuming_the_skipped_click_tells_the_shell() {
    let setup = setup(DEFAULT_TIMEOUT);
    setup.hub.with(|hub| hub.skip_next_click());
    let before = setup.changes.load(Ordering::SeqCst);
    setup.hub.on_event(A, &click(1, "#burger"));
    assert!(!setup.hub.with(|hub| hub.skip_next_armed()));
    assert_eq!(setup.changes.load(Ordering::SeqCst), before + 1);
    setup.hub.on_event(A, &click(2, "#burger"));
    assert_eq!(setup.changes.load(Ordering::SeqCst), before + 1);
}

/// A replay still under way when its view navigates: its late outcome is of the old document
/// and does not desync the view in the new one.
#[tokio::test(flavor = "multi_thread")]
async fn a_late_outcome_after_the_view_navigated_does_not_desync_it() {
    let setup = setup(Duration::from_millis(100));
    setup.replay.stall(C);
    setup.hub.on_event(A, &click(1, "button"));
    within(Duration::from_secs(1), || {
        setup.replay.calls_for(C).len() == 1
    })
    .await;
    setup
        .hub
        .with(|hub| hub.navigated(C, "http://localhost/next", Instant::now()));
    tokio::time::sleep(Duration::from_millis(250)).await;
    assert_eq!(desync_of(&setup.hub, C), None);
}

/// A view that became the leader while events for it were queued gets none of them.
#[tokio::test(flavor = "multi_thread")]
async fn a_view_that_became_the_leader_gets_no_queued_replay() {
    let setup = setup(Duration::from_millis(100));
    setup.replay.stall(C);
    setup.hub.on_event(A, &click(1, "button"));
    setup.hub.on_event(A, &click(2, "button"));
    within(Duration::from_secs(1), || {
        setup.replay.calls_for(C).len() == 1
    })
    .await;
    setup.hub.with(|hub| hub.set_active(Some(C)));
    tokio::time::sleep(Duration::from_millis(250)).await;
    assert_eq!(setup.replay.calls_for(C).len(), 1, "the second was dropped");
}

/// What an observer heard.
#[derive(Default)]
struct Heard {
    routed: Mutex<Vec<(ViewId, u64, bool)>>,
    replayed: Mutex<Vec<(ViewId, ReplayOutcome)>>,
}

impl Observer for Heard {
    fn routed(&self, source: ViewId, message: &EventMessage, route: &Route) {
        lock(&self.routed).push((source, message.seq, route.is_replay()));
    }

    fn replayed(&self, view: ViewId, _event: &SyncEvent, outcome: &ReplayOutcome) {
        lock(&self.replayed).push((view, outcome.clone()));
    }
}

/// RB-050-01: the recorder hears the leader's events with their route and every replay's
/// outcome, never a follower's event.
#[tokio::test(flavor = "multi_thread")]
async fn the_observer_hears_the_leaders_events_and_the_outcomes() {
    let setup = setup(DEFAULT_TIMEOUT);
    let heard = Arc::new(Heard::default());
    setup.hub.set_observer(Some(heard.clone()));
    setup.replay.answer(C, ReplayOutcome::NotFound);
    setup.hub.on_event(A, &click(1, "#burger"));
    setup.hub.on_event(B, &click(2, "#other"));
    within(Duration::from_secs(1), || lock(&heard.replayed).len() == 2).await;
    assert_eq!(*lock(&heard.routed), [(A, 1, true)]);
    let mut replayed = lock(&heard.replayed).clone();
    replayed.sort_by_key(|(view, _)| *view);
    assert_eq!(
        replayed,
        [(B, ReplayOutcome::Done), (C, ReplayOutcome::NotFound)]
    );
    setup.hub.set_observer(None);
    setup.hub.on_event(A, &click(3, "#burger"));
    assert_eq!(lock(&heard.routed).len(), 1);
}

/// RB-050-01 review: an outcome the hub drops - of a document the view has left since the
/// routing - is not heard either, so the recorder never drops a view from `onlyIn` for it.
#[tokio::test(flavor = "multi_thread")]
async fn the_observer_does_not_hear_a_stale_outcome() {
    let setup = setup(Duration::from_millis(100));
    let heard = Arc::new(Heard::default());
    setup.hub.set_observer(Some(heard.clone()));
    setup.replay.stall(C);
    setup.hub.on_event(A, &click(1, "button"));
    within(Duration::from_secs(1), || {
        setup.replay.calls_for(C).len() == 1 && !lock(&heard.replayed).is_empty()
    })
    .await;
    setup
        .hub
        .with(|hub| hub.navigated(C, "http://localhost/next", Instant::now()));
    tokio::time::sleep(Duration::from_millis(250)).await;
    assert_eq!(desync_of(&setup.hub, C), None);
    assert_eq!(*lock(&heard.replayed), [(B, ReplayOutcome::Done)]);
}

/// RB-050-01 review: a view desynced because it fell too far behind is heard as a timeout.
#[tokio::test(flavor = "multi_thread")]
async fn the_observer_hears_a_view_that_fell_too_far_behind() {
    let setup = setup(Duration::from_secs(30));
    let heard = Arc::new(Heard::default());
    setup.hub.set_observer(Some(heard.clone()));
    setup.replay.stall(C);
    for seq in 0..(QUEUE_DEPTH as u64 + 2) {
        setup.hub.on_event(A, &click(seq, "button"));
    }
    assert_eq!(desync_of(&setup.hub, C), Some(DesyncReason::Timeout));
    assert!(
        lock(&heard.replayed)
            .iter()
            .any(|(view, outcome)| *view == C && *outcome == ReplayOutcome::Timeout),
        "the drop is heard"
    );
}
