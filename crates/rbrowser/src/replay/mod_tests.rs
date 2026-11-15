//! The replay behind the real SyncHub: K1's latency and the desync report, with the
//! timestamping fake CDP.

use std::sync::Arc;
use std::time::{Duration, Instant};

use rb_config::SyncConfig;
use rb_engine::ViewId;
use rb_sync::{EventMessage, Hub, SyncHub};
use serde_json::json;
use tokio::runtime::Handle;

use super::fake::FakePort;
use super::metrics::summarize;
use super::*;
use crate::sync_hub::switches_from;

const LEADER: ViewId = ViewId(1);

fn hub_over(port: Arc<FakePort>, views: &[ViewId]) -> Arc<SyncHub> {
    let hub = SyncHub::new(
        Hub::new(switches_from(&SyncConfig::default())),
        Arc::new(CdpReplay::new(port)),
        Duration::from_millis(750),
        Handle::current(),
        Arc::new(|| {}),
    );
    hub.set_views(views);
    hub.with(|hub| hub.set_active(Some(LEADER)));
    hub
}

/// The leader's agent reports a click on `#b<seq>`.
fn click(seq: u64) -> EventMessage {
    serde_json::from_value(json!({
        "v": 1, "top": true, "seq": seq, "time": 0,
        "event": {
            "type": "click", "button": "left", "detail": 1, "modifiers": 0,
            "target": { "element": { "kind": "css", "path": format!("#b{seq}") },
                        "offset": { "x": 0.5, "y": 0.5 } },
        },
    }))
    .expect("a click")
}

/// When each click was released in `view`, in order.
fn released(port: &FakePort, view: ViewId) -> Vec<Instant> {
    port.sent_to(view)
        .iter()
        .filter(|sent| sent.params["type"] == "mouseReleased")
        .map(|sent| sent.at)
        .collect()
}

async fn until(mut done: impl FnMut() -> bool) {
    for _ in 0..1000 {
        if done() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("condition not reached");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn k1_a_click_reaches_every_other_view_within_150_ms_at_the_95th_percentile() {
    // 100 clicks of the leader, one every 20 ms (50 a second, far above a human), into three
    // views; each CDP command takes 2 ms like a local round trip. Measured from the hub's intake
    // (capture) to the release in the view.
    let port = Arc::new(FakePort::with_round_trip(Duration::from_millis(2)));
    let views = [LEADER, ViewId(2), ViewId(3), ViewId(4)];
    let hub = hub_over(port.clone(), &views);
    let mut captured = Vec::new();
    for seq in 1..=100 {
        captured.push(Instant::now());
        assert!(hub.on_event(LEADER, &click(seq)).is_replay());
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    until(|| {
        views[1..]
            .iter()
            .all(|view| released(&port, *view).len() == 100)
    })
    .await;
    let mut latencies = Vec::new();
    for view in &views[1..] {
        for (released, captured) in released(&port, *view).iter().zip(&captured) {
            latencies.push(released.duration_since(*captured));
        }
    }
    let summary = summarize(&latencies);
    assert_eq!(summary.samples, 300);
    assert!(summary.p95_ms < 150.0, "K1: p95 {:.1} ms", summary.p95_ms);
    assert!(port.sent_to(LEADER).is_empty(), "nothing into the leader");
    // Each view's clicks arrive in capture order: one prologue, then move, press, release.
    let first = port.sent_to(ViewId(2));
    assert!(
        first[0]
            .params
            .as_str()
            .is_some_and(|source| source.contains("#b1"))
    );
    assert!(
        first[4]
            .params
            .as_str()
            .is_some_and(|source| source.contains("#b2"))
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_view_without_the_element_is_desynced_and_the_others_click() {
    let port = Arc::new(FakePort::default());
    port.resolve_to(ViewId(3), json!({ "status": "not_found" }));
    let hub = hub_over(port.clone(), &[LEADER, ViewId(2), ViewId(3)]);
    hub.on_event(LEADER, &click(1));
    until(|| hub.with(|hub| hub.desynced()) == [ViewId(3)]).await;
    until(|| released(&port, ViewId(2)).len() == 1).await;
    assert!(released(&port, ViewId(3)).is_empty());
    // The desynced view gets no more events until it navigates or is resynced.
    hub.on_event(LEADER, &click(2));
    until(|| released(&port, ViewId(2)).len() == 2).await;
    assert_eq!(port.sent_to(ViewId(3)).len(), 1, "only the first prologue");
}

#[tokio::test(flavor = "multi_thread")]
async fn the_hub_s_submit_rule_reaches_the_prologue() {
    // Leader-only submit is on by default: every replayed input carries block_submit.
    let port = Arc::new(FakePort::default());
    let hub = hub_over(port.clone(), &[LEADER, ViewId(2)]);
    hub.on_event(LEADER, &click(1));
    until(|| released(&port, ViewId(2)).len() == 1).await;
    let prologue = &port.sent_to(ViewId(2))[0].params;
    assert!(
        prologue
            .as_str()
            .is_some_and(|source| source.contains("'submit'"))
    );
}
