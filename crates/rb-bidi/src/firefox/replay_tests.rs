use std::sync::Arc;
use std::time::{Duration, Instant};

use rb_engine::test_support::conformance::sample_spec;
use rb_engine::{Engine, EngineKind, ViewHandle, ViewId};
use rb_sync::{Replay, ReplayOutcome, ReplayRequest, SyncEvent};
use serde_json::{Value, json};

use super::*;
use crate::firefox::test_util::engine;
use crate::testing::FakeFirefox;

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
    FakeFirefox,
    Arc<FirefoxEngine>,
    Arc<dyn ViewHandle>,
    Arc<FirefoxReplay>,
) {
    let fake = FakeFirefox::start().await;
    let engine = engine(&fake, "BUNDLE();");
    let view = engine
        .create_view(&sample_spec(EngineKind::Firefox, 7))
        .await
        .expect("view");
    let replay = engine.replay();
    (fake, engine, view, replay)
}

fn agent_commands(fake: &FakeFirefox) -> Vec<String> {
    fake.calls("script.evaluate")
        .into_iter()
        .filter_map(|call| call["expression"].as_str().map(str::to_owned))
        .collect()
}

#[tokio::test]
async fn a_click_resolves_its_target_and_replays_trusted_input() {
    let (fake, _engine, _view, replay) = setup().await;
    let before = fake.commands().len();
    assert_eq!(replay.dispatch(request(click())).await, ReplayOutcome::Done);
    let methods: Vec<String> = fake.commands()[before..]
        .iter()
        .map(|(m, _)| m.clone())
        .collect();
    assert_eq!(
        methods,
        [
            "script.evaluate",
            "script.evaluate",
            "input.performActions",
            "input.performActions"
        ],
        "marker, resolve, press, release"
    );
    let expressions = agent_commands(&fake);
    let n = expressions.len();
    assert!(
        expressions[n - 2].contains("\"cmd\":\"replay\""),
        "{}",
        expressions[n - 2]
    );
    assert!(
        expressions[n - 1].contains("\"cmd\":\"resolve\""),
        "{}",
        expressions[n - 1]
    );
    let actions = fake.calls("input.performActions");
    let press = &actions[actions.len() - 2]["actions"][0]["actions"];
    assert_eq!(
        press,
        &json!([
            { "type": "pointerMove", "x": 10, "y": 20, "origin": "viewport" },
            { "type": "pointerDown", "button": 0 },
        ])
    );
}

#[tokio::test]
async fn a_missing_or_hidden_target_desyncs_without_input() {
    let (fake, _engine, _view, replay) = setup().await;
    fake.answer_agent(json!({ "status": "done" }));
    fake.answer_agent(json!({ "status": "not_found" }));
    assert_eq!(
        replay.dispatch(request(click())).await,
        ReplayOutcome::NotFound
    );
    fake.answer_agent(json!({ "status": "done" }));
    fake.answer_agent(json!({ "status": "not_visible" }));
    assert_eq!(
        replay.dispatch(request(click())).await,
        ReplayOutcome::NotVisible
    );
    assert!(fake.calls("input.performActions").is_empty());
}

#[tokio::test]
async fn a_follower_blocks_its_submit_before_the_click() {
    let (fake, _engine, _view, replay) = setup().await;
    let mut blocked = request(click());
    blocked.block_submit = true;
    assert_eq!(replay.dispatch(blocked).await, ReplayOutcome::Done);
    let expressions = agent_commands(&fake);
    let n = expressions.len();
    assert!(
        expressions[n - 1].contains("addEventListener('submit'"),
        "{}",
        expressions[n - 1]
    );
    assert!(
        expressions[n - 1].ends_with("({\"ms\":1000})"),
        "{}",
        expressions[n - 1]
    );
}

#[tokio::test]
async fn keys_replay_as_keys() {
    let (fake, _engine, _view, replay) = setup().await;
    let enter = event(
        json!({ "type": "keydown", "key": "Enter", "code": "Enter", "keyCode": 13,
                               "location": 0, "repeat": false, "modifiers": 0 }),
    );
    assert_eq!(replay.dispatch(request(enter)).await, ReplayOutcome::Done);
    let keys = fake.calls("input.performActions").pop().expect("key");
    assert_eq!(
        keys["actions"][0]["actions"],
        json!([{ "type": "keyDown", "value": "\u{E007}" }])
    );
}

/// Scroll, form state and focus go through the agent's own commands - its resolver knows role
/// and text selectors and the scroll anchor - never a weaker copy of it in the replay.
#[tokio::test]
async fn scroll_form_state_and_focus_use_the_agents_commands() {
    let (fake, _engine, _view, replay) = setup().await;
    let role = json!({ "element": { "kind": "role", "role": "button", "name": "Go", "path": "#go" },
                       "offset": { "x": 0.5, "y": 0.5 } });
    let scroll = event(
        json!({ "type": "scroll", "anchor": role, "x": 0, "y": 300, "maxX": 0, "maxY": 600 }),
    );
    assert_eq!(replay.dispatch(request(scroll)).await, ReplayOutcome::Done);
    let last = agent_commands(&fake).pop().expect("scroll");
    assert!(last.contains("__rbAgent.command("), "{last}");
    assert!(last.contains("\"cmd\":\"scroll\""), "{last}");
    assert!(last.contains("\"anchor\":{"), "{last}");
    assert!(last.contains("\"kind\":\"role\""), "{last}");

    let input = event(
        json!({ "type": "input", "inputType": "insertText", "data": "a", "value": "a", "target": role }),
    );
    assert_eq!(replay.dispatch(request(input)).await, ReplayOutcome::Done);
    let last = agent_commands(&fake).pop().expect("input");
    assert!(last.contains("\"cmd\":\"apply\""), "{last}");
    assert!(last.contains("\"value\":\"a\""), "{last}");

    let change = event(json!({ "type": "change", "checked": true, "target": target() }));
    assert_eq!(replay.dispatch(request(change)).await, ReplayOutcome::Done);
    let last = agent_commands(&fake).pop().expect("change");
    assert!(last.contains("\"cmd\":\"apply\""), "{last}");
    assert!(last.contains("\"checked\":true"), "{last}");

    let focus = event(json!({ "type": "focus", "target": target() }));
    assert_eq!(replay.dispatch(request(focus)).await, ReplayOutcome::Done);
    let last = agent_commands(&fake).pop().expect("focus");
    assert!(last.contains("\"cmd\":\"focus\""), "{last}");

    // A file input's change carries no state: nothing is sent, nothing desyncs.
    let before = agent_commands(&fake).len();
    let file = event(json!({ "type": "change", "target": target() }));
    assert_eq!(replay.dispatch(request(file)).await, ReplayOutcome::Done);
    assert!(
        agent_commands(&fake)[before..]
            .iter()
            .all(|expression| !expression.contains("\"cmd\":\"apply\"")),
        "no apply for a file input"
    );
    assert!(
        agent_commands(&fake)
            .iter()
            .all(|expression| !expression.contains("rbFind")),
        "no resolver of the replay's own"
    );
}

#[tokio::test]
async fn a_missing_control_desyncs() {
    let (fake, _engine, _view, replay) = setup().await;
    fake.answer_agent(json!({ "status": "done" }));
    fake.answer_agent(json!({ "status": "not_found" }));
    let focus = event(json!({ "type": "focus", "target": target() }));
    assert_eq!(
        replay.dispatch(request(focus)).await,
        ReplayOutcome::NotFound
    );
}

fn key(kind: &str, key: &str, code: &str, modifiers: u8) -> SyncEvent {
    event(
        json!({ "type": kind, "key": key, "code": code, "keyCode": 0, "location": 0,
                  "repeat": false, "modifiers": modifiers }),
    )
}

fn keyboard_actions(fake: &FakeFirefox) -> Vec<Value> {
    fake.calls("input.performActions")
        .into_iter()
        .flat_map(|call| call["actions"].as_array().cloned().unwrap_or_default())
        .filter(|source| source["type"] == "key")
        .flat_map(|source| source["actions"].as_array().cloned().unwrap_or_default())
        .collect()
}

/// A modifier whose keyup never came (the leader lost the focus) is let go before the next key
/// that says it is up: Firefox holds pressed keys until they are released.
#[tokio::test]
async fn a_modifier_is_released_when_the_next_key_says_it_is_up() {
    let (fake, _engine, _view, replay) = setup().await;
    let shift = 8; // MOD_SHIFT
    assert_eq!(
        replay
            .dispatch(request(key("keydown", "Shift", "ShiftLeft", shift)))
            .await,
        ReplayOutcome::Done
    );
    // No keyup of Shift: the next key comes without it.
    assert_eq!(
        replay
            .dispatch(request(key("keydown", "a", "KeyA", 0)))
            .await,
        ReplayOutcome::Done
    );
    let actions = keyboard_actions(&fake);
    assert_eq!(
        actions,
        vec![
            json!({ "type": "keyDown", "value": "\u{E008}" }),
            json!({ "type": "keyUp", "value": "\u{E008}" }),
            json!({ "type": "keyDown", "value": "a" }),
        ]
    );
}

/// A key that says a modifier is down while Firefox does not hold it presses it first.
#[tokio::test]
async fn a_modifier_the_key_needs_is_pressed_first() {
    let (fake, _engine, _view, replay) = setup().await;
    let ctrl = 2; // MOD_CTRL
    assert_eq!(
        replay
            .dispatch(request(key("keydown", "a", "KeyA", ctrl)))
            .await,
        ReplayOutcome::Done
    );
    assert_eq!(
        keyboard_actions(&fake),
        vec![
            json!({ "type": "keyDown", "value": "\u{E009}" }),
            json!({ "type": "keyDown", "value": "a" }),
        ]
    );
}

/// Another leader starts from nothing: what the last one held is released first.
#[tokio::test]
async fn a_new_leader_releases_what_the_last_one_held() {
    let (fake, _engine, _view, replay) = setup().await;
    let shift = 8;
    replay
        .dispatch(request(key("keydown", "Shift", "ShiftLeft", shift)))
        .await;
    assert!(fake.calls("input.releaseActions").is_empty());
    let mut other = request(key("keydown", "a", "KeyA", 0));
    other.source = ViewId(2);
    assert_eq!(replay.dispatch(other).await, ReplayOutcome::Done);
    assert_eq!(fake.calls("input.releaseActions").len(), 1);
}

#[tokio::test]
async fn what_firefox_does_not_replay_is_skipped_not_desynced() {
    let (fake, _engine, _view, replay) = setup().await;
    let pointer = event(
        json!({ "type": "pointer", "phase": "down", "button": "left", "buttons": 1,
                                 "pointerType": "mouse", "modifiers": 0, "target": target() }),
    );
    assert_eq!(
        replay.dispatch(request(pointer)).await,
        ReplayOutcome::Unsupported
    );
    let submit = event(
        json!({ "type": "submit", "method": "post", "action": "http://x/", "target": target() }),
    );
    assert_eq!(
        replay.dispatch(request(submit)).await,
        ReplayOutcome::Unsupported
    );
    let mut framed = request(click());
    framed.top = false;
    assert_eq!(replay.dispatch(framed).await, ReplayOutcome::Unsupported);
    let mut elsewhere = request(click());
    elsewhere.target = ViewId(99);
    assert_eq!(replay.dispatch(elsewhere).await, ReplayOutcome::Unsupported);
    assert!(fake.calls("input.performActions").is_empty());
}

/// K11, the part a unit test can show: rBrowser's own share of a Firefox follow - a scroll
/// replay and a navigation, each over a real WebSocket - stays far under 300 ms at p95 over 100
/// rounds. The live measurement against Firefox is the Windows acceptance.
#[tokio::test]
async fn k11_rbrowsers_share_of_a_follow_is_far_under_300_ms() {
    let (_fake, _engine, view, replay) = setup().await;
    let mut scroll = Vec::new();
    let mut navigation = Vec::new();
    for round in 0..100 {
        let y = f64::from(round) * 10.0;
        let event = event(json!({ "type": "scroll", "x": 0, "y": y, "maxX": 0, "maxY": 1000 }));
        let start = Instant::now();
        assert_eq!(replay.dispatch(request(event)).await, ReplayOutcome::Done);
        scroll.push(start.elapsed());
        let start = Instant::now();
        view.navigate(&format!("http://localhost/fixture/{round}"))
            .await
            .expect("navigate");
        navigation.push(start.elapsed());
    }
    for (what, mut times) in [("scroll", scroll), ("navigation", navigation)] {
        times.sort();
        let p95 = times[94];
        // K11's own measurement: a p95 over 100 local rounds, two orders of magnitude of headroom.
        // wall-clock: allow (K11's p95 with 300x headroom)
        assert!(p95 < Duration::from_millis(300), "{what} p95 {p95:?}");
    }
}
