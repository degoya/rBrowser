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
