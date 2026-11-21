use std::sync::Arc;

use rb_engine::test_support::conformance::sample_spec;
use rb_engine::{Engine, EngineKind, ViewHandle, ViewId};
use rb_sync::{Replay, ReplayOutcome, ReplayRequest, SyncEvent};
use serde_json::{Value, json};

use super::*;
use crate::driver::Attached;
use crate::testing::FakeSafari;

const TARGET: &str = r##"{"element":{"kind":"css","path":"#go"},"offset":{"x":0.5,"y":0.5}}"##;

fn event(value: Value) -> SyncEvent {
    serde_json::from_value(value).expect("a sync event")
}

fn target() -> Value {
    serde_json::from_str(TARGET).expect("target")
}

fn request(event: SyncEvent) -> ReplayRequest {
    ReplayRequest {
        source: ViewId(1),
        target: ViewId(7),
        top: true,
        frame: None,
        event: event.into(),
        block_submit: false,
    }
}

fn click() -> SyncEvent {
    event(
        json!({ "type": "click", "button": "left", "detail": 1, "modifiers": 0, "target": target() }),
    )
}

async fn setup() -> (
    FakeSafari,
    Arc<SafariEngine>,
    Arc<dyn ViewHandle>,
    SafariReplay,
) {
    let fake = FakeSafari::start().await;
    let engine = SafariEngine::new(Arc::new(Attached(fake.port())), "BUNDLE();");
    let view = engine
        .create_view(&sample_spec(EngineKind::Safari, 7))
        .await
        .expect("a view");
    let replay = SafariReplay::new(&engine);
    (fake, engine, view, replay)
}

#[tokio::test]
async fn a_click_resolves_its_target_and_replays_real_input_at_its_point() {
    let (fake, _engine, _view, replay) = setup().await;
    assert_eq!(replay.dispatch(request(click())).await, ReplayOutcome::Done);
    assert_eq!(fake.agent_commands(), ["replay", "resolve"]);
    let actions = fake.actions();
    assert_eq!(actions.len(), 2, "press and release");
    let press = &actions[0][0];
    assert_eq!(press["type"], "pointer");
    assert_eq!(press["actions"][0]["type"], "pointerMove");
    assert_eq!(press["actions"][0]["x"], 10);
    assert_eq!(press["actions"][0]["y"], 20);
    assert_eq!(press["actions"][0]["origin"], "viewport");
    assert_eq!(press["actions"][1]["type"], "pointerDown");
    assert_eq!(actions[1][0]["actions"][1]["type"], "pointerUp");
}

#[tokio::test]
async fn the_agent_boots_on_demand_in_a_new_document() {
    let (fake, _engine, view, replay) = setup().await;
    let boots = || {
        fake.requests_to("/execute/sync")
            .into_iter()
            .filter(|request| {
                request.body["script"]
                    .as_str()
                    .is_some_and(|script| script.contains("BUNDLE();"))
            })
            .count()
    };
    assert_eq!(replay.dispatch(request(click())).await, ReplayOutcome::Done);
    assert_eq!(boots(), 1, "booted once for the first command");
    assert_eq!(replay.dispatch(request(click())).await, ReplayOutcome::Done);
    assert_eq!(boots(), 1, "the document keeps its agent");
    view.navigate("http://localhost/next")
        .await
        .expect("navigated");
    assert_eq!(replay.dispatch(request(click())).await, ReplayOutcome::Done);
    assert_eq!(boots(), 2, "a new document boots it again");
}

#[tokio::test]
async fn a_missing_target_desyncs_and_sends_no_input() {
    let (fake, _engine, _view, replay) = setup().await;
    fake.answer_agent(json!({ "status": "done" }));
    fake.answer_agent(json!({ "status": "not_found" }));
    assert_eq!(
        replay.dispatch(request(click())).await,
        ReplayOutcome::NotFound
    );
    assert!(fake.actions().is_empty());
}

#[tokio::test]
async fn scroll_input_and_focus_are_the_agent_s_commands() {
    let (fake, _engine, _view, replay) = setup().await;
    let scroll = event(json!({ "type": "scroll", "x": 0, "y": 300, "maxX": 0, "maxY": 600 }));
    let outcome = replay.dispatch(request(scroll)).await;
    assert_eq!(outcome, ReplayOutcome::Done, "{outcome:?}");
    let focus = event(json!({ "type": "focus", "target": target() }));
    assert_eq!(replay.dispatch(request(focus)).await, ReplayOutcome::Done);
    assert_eq!(
        fake.agent_commands(),
        ["replay", "scroll", "replay", "focus"]
    );
    assert!(fake.actions().is_empty());
}

#[tokio::test]
async fn keys_replay_as_webdriver_keys() {
    let (fake, _engine, _view, replay) = setup().await;
    let down = event(json!({
        "type": "keydown", "key": "a", "code": "KeyA", "keyCode": 65,
        "location": 0, "repeat": false, "modifiers": 0
    }));
    assert_eq!(replay.dispatch(request(down)).await, ReplayOutcome::Done);
    let actions = fake.actions();
    assert_eq!(actions[0][0]["type"], "key");
    assert_eq!(
        actions[0][0]["actions"],
        json!([{ "type": "keyDown", "value": "a" }])
    );
}

#[tokio::test]
async fn iframe_events_and_unknown_views_are_not_replayed() {
    let (fake, _engine, _view, replay) = setup().await;
    let mut inner = request(click());
    inner.top = false;
    assert_eq!(replay.dispatch(inner).await, ReplayOutcome::Unsupported);
    let mut elsewhere = request(click());
    elsewhere.target = ViewId(99);
    assert_eq!(replay.dispatch(elsewhere).await, ReplayOutcome::Unsupported);
    assert!(fake.actions().is_empty());
}

#[tokio::test]
async fn another_leader_releases_what_was_held() {
    let (fake, _engine, _view, replay) = setup().await;
    assert_eq!(replay.dispatch(request(click())).await, ReplayOutcome::Done);
    let mut other = request(click());
    other.source = ViewId(2);
    assert_eq!(replay.dispatch(other).await, ReplayOutcome::Done);
    let released = fake
        .requests_to("/actions")
        .into_iter()
        .filter(|request| request.method == "DELETE")
        .count();
    assert_eq!(released, 1);
}

#[tokio::test]
async fn on_ios_a_click_is_a_touch_and_hover_is_not_replayed() {
    let fake = FakeSafari::start().await;
    let tools = Arc::new(crate::ios::fake::FakeIosTools::with_xcode());
    let engine = SafariEngine::ios(
        Arc::new(Attached(fake.port())),
        tools as Arc<dyn crate::ios::IosTools>,
        "BUNDLE();",
    );
    engine
        .create_view(&sample_spec(EngineKind::Ios, 7))
        .await
        .expect("a view");
    let replay = SafariReplay::new(&engine);
    assert_eq!(replay.dispatch(request(click())).await, ReplayOutcome::Done);
    let actions = fake.actions();
    assert_eq!(actions.len(), 2, "press and release");
    assert_eq!(actions[0][0]["parameters"]["pointerType"], "touch");
    let hover = event(json!({ "type": "hover", "target": target() }));
    assert_eq!(
        replay.dispatch(request(hover)).await,
        ReplayOutcome::Unsupported
    );
    assert_eq!(fake.actions().len(), 2, "no hover on a phone");
}
