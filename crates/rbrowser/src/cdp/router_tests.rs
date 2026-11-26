use super::*;
use crate::cdp::fake::FakeBrowser;

/// A transport that never answers.
struct Silent;

impl CdpTransport for Silent {
    fn send(&self, _frame: Vec<u8>) -> Result<(), EngineError> {
        Ok(())
    }
}

#[tokio::test]
async fn a_call_gets_its_result_and_carries_the_session() {
    let (fake, router) = FakeBrowser::connect();
    fake.add_target("T1", "about:blank#rb-view-1");
    let targets = router
        .call("Target.getTargets", json!({}), None)
        .await
        .expect("answered");
    assert_eq!(targets["targetInfos"][0]["targetId"], "T1");
    router
        .call("Page.reload", json!({}), Some("S9"))
        .await
        .expect("answered");
    assert_eq!(fake.sent("Page.reload")[0]["sessionId"], "S9");
    assert!(fake.sent("Target.getTargets")[0].get("sessionId").is_none());
}

#[tokio::test]
async fn an_error_reply_becomes_a_cdp_error() {
    let router = CdpRouter::new(Arc::new(Silent));
    let call = router.call("Page.navigate", json!({"url": "x"}), Some("S1"));
    let answer = async {
        tokio::task::yield_now().await;
        router.on_message(json!({"id": 1, "error": {"code": -32000, "message": "Invalid url"}}));
    };
    let (result, ()) = tokio::join!(call, answer);
    match result {
        Err(EngineError::Cdp {
            method,
            code,
            message,
        }) => {
            assert_eq!((method.as_str(), code), ("Page.navigate", -32000));
            assert_eq!(message, "Invalid url");
        }
        other => panic!("expected a CDP error, got {other:?}"),
    }
}

/// RB-099-19: the result is taken out of the message, not copied - the caller gets it whole,
/// and a reply without one answers null.
#[tokio::test]
async fn a_reply_hands_over_its_whole_result_or_null() {
    let router = CdpRouter::new(Arc::new(Silent));
    let result = json!({"tree": {"children": [{"id": 1}, {"id": 2}]}, "data": "x".repeat(4096)});
    let call = router.call("DOM.getDocument", json!({}), None);
    let answer = async {
        tokio::task::yield_now().await;
        router.on_message(json!({"id": 1, "result": result.clone(), "sessionId": "S1"}));
    };
    let (got, ()) = tokio::join!(call, answer);
    assert_eq!(got.expect("answered"), result);
    let call = router.call("Page.enable", json!({}), None);
    let answer = async {
        tokio::task::yield_now().await;
        router.on_message(json!({"id": 2}));
    };
    let (got, ()) = tokio::join!(call, answer);
    assert_eq!(got.expect("answered"), Value::Null);
}

#[tokio::test]
async fn events_reach_every_subscriber_with_their_session() {
    let (fake, router) = FakeBrowser::connect();
    let mut first = router.subscribe();
    let mut second = router.subscribe();
    fake.emit(json!({"method": "Target.detachedFromTarget", "params": {"sessionId": "S1"}, "sessionId": null}));
    fake.emit(json!({"method": "Page.loadEventFired", "params": {}, "sessionId": "S2"}));
    let event = first.recv().await.expect("event");
    assert_eq!(event.method, "Target.detachedFromTarget");
    assert_eq!(event.session_id, None);
    let _ = second.recv().await.expect("event");
    let event = second.recv().await.expect("event");
    assert_eq!(event.session_id.as_deref(), Some("S2"));
}

#[tokio::test]
async fn closing_fails_waiting_and_later_calls() {
    let router = CdpRouter::new(Arc::new(Silent));
    let call = router.call("Browser.getVersion", json!({}), None);
    let close = async {
        tokio::task::yield_now().await;
        router.on_closed();
    };
    let (result, ()) = tokio::join!(call, close);
    assert!(matches!(result, Err(EngineError::Closed)), "{result:?}");
    assert!(router.is_closed());
    assert!(matches!(
        router.call("Browser.getVersion", json!({}), None).await,
        Err(EngineError::Closed)
    ));
}

#[tokio::test]
async fn a_tap_gets_its_methods_events_on_its_own_channel_and_counts_what_it_dropped() {
    let router = CdpRouter::new(Arc::new(Silent));
    let mut tap = router.tap("Fetch.requestPaused", 2);
    let mut all = router.subscribe();
    for id in ["R1", "R2", "R3"] {
        router.on_message(json!({ "method": "Fetch.requestPaused", "sessionId": "S1",
                                  "params": { "requestId": id } }));
    }
    router.on_message(json!({ "method": "Page.loadEventFired", "params": {} }));
    let first = tap.recv().await.expect("an event");
    assert_eq!(first.params["requestId"], "R1");
    assert_eq!(first.session_id.as_deref(), Some("S1"));
    assert_eq!(
        tap.recv().await.expect("an event").params["requestId"],
        "R2"
    );
    assert_eq!(tap.take_dropped(), 1, "the third did not fit");
    assert_eq!(tap.take_dropped(), 0, "taking resets the count");
    assert_eq!(
        all.recv().await.expect("broadcast too").method,
        "Fetch.requestPaused"
    );
    drop(tap);
    router.on_message(json!({ "method": "Fetch.requestPaused", "params": {} }));
    assert!(lock(&router.taps).is_empty(), "a dropped tap is let go");
}

fn frame() -> Value {
    json!({ "method": "Page.screencastFrame", "sessionId": "S1",
            "params": { "data": "AAAA", "sessionId": 1 } })
}

#[tokio::test]
async fn a_burst_of_screencast_frames_crowds_out_neither_a_tap_nor_the_broadcast() {
    let router = CdpRouter::new(Arc::new(Silent));
    let mut tap = router.tap_methods(
        &["Runtime.executionContextCreated", "Runtime.bindingCalled"],
        4,
    );
    let mut all = router.subscribe();
    router.on_message(
        json!({ "method": "Runtime.executionContextCreated", "sessionId": "S1",
                              "params": { "context": { "id": 7 } } }),
    );
    for _ in 0..(EVENT_BUFFER * 3) {
        router.on_message(frame());
    }
    router.on_message(
        json!({ "method": "Runtime.bindingCalled", "sessionId": "S1",
                              "params": { "name": "__rbEmit" } }),
    );
    let first = tap.recv_shared().await.expect("an event");
    assert_eq!(first.method, "Runtime.executionContextCreated");
    let second = tap.recv_shared().await.expect("an event");
    assert_eq!(
        second.method, "Runtime.bindingCalled",
        "the order of its methods holds"
    );
    assert_eq!(tap.take_dropped(), 0);
    let event = all
        .recv()
        .await
        .expect("no lag: the frames are not in the broadcast");
    assert!(Arc::ptr_eq(&event, &first), "one event, shared, not copied");
    assert_eq!(
        all.recv().await.expect("an event").method,
        "Runtime.bindingCalled"
    );
}

#[tokio::test]
async fn screencast_frames_reach_taps_and_the_raw_broadcast_only() {
    let router = CdpRouter::new(Arc::new(Silent));
    let mut tap = router.tap("Page.screencastFrame", 2);
    let mut raw = router.subscribe_raw();
    let mut all = router.subscribe();
    router.on_message(frame());
    router.on_message(json!({ "method": "Page.loadEventFired", "params": {} }));
    let owned = tap.recv().await.expect("the tap gets the frame");
    assert_eq!(owned.params["data"], "AAAA");
    assert_eq!(
        raw.recv().await.expect("raw").method,
        "Page.screencastFrame"
    );
    assert_eq!(raw.recv().await.expect("raw").method, "Page.loadEventFired");
    assert_eq!(
        all.recv().await.expect("an event").method,
        "Page.loadEventFired"
    );
    assert!(all.try_recv().is_err(), "no frame in the shared broadcast");
}

/// RB-090-34: one event, n receivers, one allocation - every subscriber, the raw broadcast and a
/// tap hold the same `Arc`, counted by its strong count.
#[tokio::test]
async fn one_event_reaches_n_receivers_as_one_allocation() {
    const RECEIVERS: usize = 6;
    let router = CdpRouter::new(Arc::new(Silent));
    let mut subscribers: Vec<_> = (0..RECEIVERS).map(|_| router.subscribe()).collect();
    let mut raw = router.subscribe_raw();
    let mut tap = router.tap("Page.loadEventFired", 1);
    router.on_message(json!({ "method": "Page.loadEventFired", "sessionId": "S1",
                              "params": { "timestamp": 1.5 } }));
    let first = tap.recv_shared().await.expect("the tap gets it");
    let mut held = vec![raw.recv().await.expect("the raw broadcast gets it")];
    for subscriber in &mut subscribers {
        held.push(subscriber.recv().await.expect("every subscriber gets it"));
    }
    assert!(
        held.iter().all(|event| Arc::ptr_eq(event, &first)),
        "the same allocation everywhere"
    );
    // Beside the receivers' handles only the two broadcasts' slots may still hold it (tokio
    // lets a slot go when its last receiver read it or when it is overwritten): no copy.
    let receivers = held.len() + 1;
    assert!((receivers..=receivers + 2).contains(&Arc::strong_count(&first)));
    drop(held);
    assert!((1..=3).contains(&Arc::strong_count(&first)));
}
