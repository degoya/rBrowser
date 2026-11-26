use std::sync::Arc;

use rb_engine::ViewId;
use serde_json::{Value, json};

use super::*;
use crate::cdp::CdpRouter;
use crate::cdp::fake::FakeBrowser;
use crate::time_control::model::{Clock, ClockStart};
use crate::time_control::script::{RESET_EXPRESSION, install_source};

const NOW: i64 = 1_795_176_000_000;

fn setup() -> (Arc<FakeBrowser>, Arc<CdpRouter>, TimeHooks) {
    let (fake, router) = FakeBrowser::connect();
    (fake, router, TimeHooks::default())
}

fn fixed() -> Clock {
    Clock::starting(ClockStart::Instant(1_798_102_800_000), 0.0, NOW)
}

/// The messages of `method` sent in `session`.
fn sent_in(fake: &FakeBrowser, method: &str, session: &str) -> Vec<Value> {
    fake.sent(method)
        .into_iter()
        .filter(|message| message["sessionId"] == session)
        .collect()
}

fn scripts_in(fake: &FakeBrowser, session: &str) -> Vec<Value> {
    sent_in(fake, "Page.addScriptToEvaluateOnNewDocument", session)
}

#[tokio::test(flavor = "multi_thread")]
async fn an_attach_with_nothing_changed_sends_nothing() {
    let (fake, router, hooks) = setup();
    hooks.attach(&router, ViewId(1), "S1").await;
    assert!(scripts_in(&fake, "S1").is_empty());
    assert!(sent_in(&fake, "Animation.setPlaybackRate", "S1").is_empty());
    assert_eq!(hooks.clock(), None);
    assert_eq!(hooks.rates(), [(ViewId(1), 1.0)]);
}

#[tokio::test(flavor = "multi_thread")]
async fn the_clock_goes_into_every_view_at_once_and_into_every_view_attached_later() {
    let (fake, router, hooks) = setup();
    hooks.attach(&router, ViewId(1), "S1").await;
    hooks.attach(&router, ViewId(2), "S2").await;
    let outcomes = hooks.set_clock(&router, Some(fixed())).await;
    assert_eq!(outcomes, [(ViewId(1), Ok(())), (ViewId(2), Ok(()))]);
    for session in ["S1", "S2"] {
        let scripts = scripts_in(&fake, session);
        assert_eq!(scripts.len(), 1, "{session}");
        let params = &scripts[0]["params"];
        // The page's main world, before its scripts, and at once in the document there.
        assert!(params.get("worldName").is_none());
        assert_eq!(params["runImmediately"], true);
        assert_eq!(params["source"], install_source(&fixed()));
        assert_eq!(sent_in(&fake, "Page.enable", session).len(), 1);
    }
    hooks.attach(&router, ViewId(3), "S3").await;
    assert_eq!(scripts_in(&fake, "S3").len(), 1);
    assert_eq!(hooks.clock(), Some(fixed()));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_new_clock_replaces_the_script_and_the_reset_takes_it_back() {
    let (fake, router, hooks) = setup();
    hooks.attach(&router, ViewId(1), "S1").await;
    hooks.set_clock(&router, Some(fixed())).await;
    let faster = fixed().with_speed(2.0, NOW + 1_000);
    hooks.set_clock(&router, Some(faster)).await;
    let removed = sent_in(&fake, "Page.removeScriptToEvaluateOnNewDocument", "S1");
    assert_eq!(removed.len(), 1);
    assert_eq!(removed[0]["params"]["identifier"], "1");
    assert_eq!(
        scripts_in(&fake, "S1")[1]["params"]["source"],
        install_source(&faster)
    );
    let outcomes = hooks.set_clock(&router, None).await;
    assert_eq!(outcomes, [(ViewId(1), Ok(()))]);
    let removed = sent_in(&fake, "Page.removeScriptToEvaluateOnNewDocument", "S1");
    assert_eq!(removed[1]["params"]["identifier"], "2");
    let reset = sent_in(&fake, "Runtime.evaluate", "S1");
    assert_eq!(reset.len(), 1);
    assert_eq!(reset[0]["params"]["expression"], RESET_EXPRESSION);
    assert!(reset[0]["params"].get("contextId").is_none());
    assert_eq!(hooks.clock(), None);
    // A view attached now gets no clock.
    hooks.attach(&router, ViewId(2), "S2").await;
    assert!(scripts_in(&fake, "S2").is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_rate_goes_to_its_views_and_again_with_a_new_session() {
    let (fake, router, hooks) = setup();
    hooks.attach(&router, ViewId(1), "S1").await;
    hooks.attach(&router, ViewId(2), "S2").await;
    let outcomes = hooks.set_rate(&router, &[ViewId(1)], 0.0).await;
    assert_eq!(outcomes, [(ViewId(1), Ok(()))]);
    let sent = sent_in(&fake, "Animation.setPlaybackRate", "S1");
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0]["params"], json!({ "playbackRate": 0.0 }));
    assert!(sent_in(&fake, "Animation.setPlaybackRate", "S2").is_empty());
    assert_eq!(hooks.rates(), [(ViewId(1), 0.0), (ViewId(2), 1.0)]);
    assert_eq!(hooks.held_rate("S1"), Some(0.0));
    assert_eq!(hooks.held_rate("S2"), None);
    // The session was lost and came back: the rate goes again.
    hooks.attach(&router, ViewId(1), "S9").await;
    assert_eq!(sent_in(&fake, "Animation.setPlaybackRate", "S9").len(), 1);
    assert_eq!(hooks.held_rate("S1"), None);
    assert_eq!(hooks.held_rate("S9"), Some(0.0));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_view_not_attached_yet_gets_its_rate_at_the_attach() {
    let (fake, router, hooks) = setup();
    let outcomes = hooks.set_rate(&router, &[ViewId(4)], 0.25).await;
    assert_eq!(outcomes, [(ViewId(4), Ok(()))]);
    assert!(fake.sent("Animation.setPlaybackRate").is_empty());
    hooks.attach(&router, ViewId(4), "S4").await;
    let sent = sent_in(&fake, "Animation.setPlaybackRate", "S4");
    assert_eq!(sent[0]["params"], json!({ "playbackRate": 0.25 }));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_refused_call_is_the_views_failure_and_the_wish_stays() {
    let (fake, router, hooks) = setup();
    hooks.attach(&router, ViewId(1), "S1").await;
    fake.fail("Animation.setPlaybackRate", true);
    let outcomes = hooks.set_rate(&router, &[ViewId(1)], 0.0).await;
    assert_eq!(outcomes.len(), 1);
    assert!(outcomes[0].1.is_err(), "{outcomes:?}");
    assert_eq!(hooks.rates(), [(ViewId(1), 0.0)]);
    fake.fail("Page.addScriptToEvaluateOnNewDocument", true);
    let outcomes = hooks.set_clock(&router, Some(fixed())).await;
    assert!(outcomes[0].1.is_err(), "{outcomes:?}");
    assert_eq!(hooks.clock(), Some(fixed()));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_closed_view_is_forgotten() {
    let (_fake, router, hooks) = setup();
    hooks.attach(&router, ViewId(1), "S1").await;
    hooks.set_rate(&router, &[ViewId(1)], 0.0).await;
    hooks.forget(ViewId(1));
    assert!(hooks.rates().is_empty());
    assert_eq!(hooks.held_rate("S1"), None);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_step_runs_in_the_world_it_is_given_and_answers_how_many_moved() {
    let (fake, router, _hooks) = setup();
    fake.answer_with(
        "Runtime.evaluate",
        json!({ "result": { "type": "number", "value": 2 } }),
    );
    assert_eq!(step(&router, "S1", Some(7), 100).await, Ok(2));
    let sent = sent_in(&fake, "Runtime.evaluate", "S1");
    assert_eq!(sent[0]["params"]["contextId"], 7);
    assert_eq!(sent[0]["params"]["returnByValue"], true);
    assert!(
        sent[0]["params"]["expression"]
            .as_str()
            .is_some_and(|expression| expression.ends_with(")(100)"))
    );
    // Without the agent's world: the page's own.
    assert_eq!(step(&router, "S2", None, 100).await, Ok(2));
    assert!(
        sent_in(&fake, "Runtime.evaluate", "S2")[0]["params"]
            .get("contextId")
            .is_none()
    );
    fake.answer_with(
        "Runtime.evaluate",
        json!({ "exceptionDetails": { "text": "Uncaught" } }),
    );
    assert!(step(&router, "S1", Some(7), 100).await.is_err());
}

/// RB-099-19: a follower that lagged keeps only the views the host still has - asked per view
/// the hooks know, under their lock, so an attach cannot slip in between (review w39).
#[tokio::test(flavor = "multi_thread")]
async fn forgetting_the_gone_views_asks_for_each_known_view() {
    let (_fake, router, hooks) = setup();
    for (view, session) in [(1, "S1"), (2, "S2"), (3, "S3")] {
        hooks.attach(&router, ViewId(view), session).await;
    }
    let asked = std::sync::Mutex::new(Vec::new());
    assert!(hooks.forget_gone(|view| {
        lock(&asked).push(view);
        view != ViewId(2)
    }));
    assert_eq!(*lock(&asked), [ViewId(1), ViewId(2), ViewId(3)]);
    assert_eq!(hooks.rates(), [(ViewId(1), 1.0), (ViewId(3), 1.0)]);
    assert!(!hooks.forget_gone(|_| true), "nothing more to forget");
}
