//! RB-080-27: one event to eight views clones its payload once. The routed requests, the
//! queued items and the replays share one `Arc<SyncEvent>` (and one frame path), so a keystroke
//! with a `value` is not copied per view.

use std::sync::Arc;
use std::time::{Duration, Instant};

use rb_engine::ViewId;
use tokio::runtime::Handle;

use super::*;
use crate::driver::{DEFAULT_TIMEOUT, SyncHub};
use crate::event::{InputData, SyncEvent};
use crate::replay::RecordingReplay;
use crate::switches::SyncSwitches;
use crate::switches::tests::target;

const LEADER: ViewId = ViewId(1);
const VIEWS: [ViewId; 9] = [
    ViewId(1),
    ViewId(2),
    ViewId(3),
    ViewId(4),
    ViewId(5),
    ViewId(6),
    ViewId(7),
    ViewId(8),
    ViewId(9),
];

fn typed(seq: u64) -> EventMessage {
    EventMessage {
        v: 1,
        top: true,
        frame: Some(vec!["main > iframe".into()]),
        seq,
        time: 0,
        path: Vec::new(),
        event: SyncEvent::Input(InputData {
            input_type: "insertText".into(),
            data: Some("x".into()),
            value: Some("a long value typed into a field".into()),
            sensitive: false,
            target: target("input[name=q]"),
        }),
    }
}

fn hub_with_eight_followers() -> Hub {
    let mut hub = Hub::new(SyncSwitches::default());
    hub.set_views(&VIEWS);
    hub.set_active(Some(LEADER));
    hub
}

#[test]
fn the_route_to_eight_views_shares_one_event_and_one_frame_path() {
    let mut hub = hub_with_eight_followers();
    let message = typed(1);
    let Route::Replay(requests) = hub.route(LEADER, &message, Instant::now()) else {
        panic!("replayed");
    };
    assert_eq!(requests.len(), 8);
    let first = &requests[0];
    assert_eq!(*first.event, message.event);
    assert_eq!(
        first.frame.as_deref().map(<[String]>::to_vec),
        message.frame
    );
    for request in &requests {
        assert!(Arc::ptr_eq(&first.event, &request.event));
        let (Some(frame), Some(shared)) = (&first.frame, &request.frame) else {
            panic!("the frame path is kept");
        };
        assert!(Arc::ptr_eq(frame, shared));
    }
    // One payload in memory, held by the eight requests and nothing else.
    assert_eq!(Arc::strong_count(&first.event), 8);
}

#[tokio::test(flavor = "multi_thread")]
async fn the_replays_get_the_routed_event_itself_not_a_copy() {
    let replay = Arc::new(RecordingReplay::default());
    let hub = SyncHub::new(
        hub_with_eight_followers(),
        replay.clone(),
        DEFAULT_TIMEOUT,
        Handle::current(),
        Arc::new(|| {}),
    );
    let Route::Replay(requests) = hub.on_event(LEADER, &typed(1)) else {
        panic!("replayed");
    };
    tokio::time::timeout(Duration::from_secs(5), async {
        while replay.calls().len() < 8 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("eight replays");
    let event = &requests[0].event;
    for call in replay.calls() {
        assert!(Arc::ptr_eq(event, &call.event), "view {}", call.target);
    }
}
