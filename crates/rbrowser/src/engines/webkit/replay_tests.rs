use std::sync::Arc;

use rb_config::EngineKind;
use rb_engine::test_support::conformance::sample_spec;
use rb_engine::{Engine, ViewId};
use rb_sync::{Replay, ReplayOutcome, ReplayRequest, SyncEvent};
use serde_json::{Value, json};
use tokio::runtime::Handle;

use super::*;
use crate::engines::webkit::fake::FakeSidecar;
use crate::engines::webkit::rpc;

fn target() -> Value {
    json!({ "element": { "kind": "css", "path": "main > button" }, "offset": { "x": 0.5, "y": 0.5 } })
}

fn event(value: Value) -> SyncEvent {
    serde_json::from_value(value).expect("an event")
}

fn request(event: SyncEvent) -> ReplayRequest {
    ReplayRequest {
        source: ViewId(1),
        target: ViewId(3),
        top: true,
        frame: None,
        event: event.into(),
        block_submit: false,
    }
}

fn click(detail: u32) -> SyncEvent {
    event(
        json!({ "type": "click", "button": "left", "detail": detail, "modifiers": 2, "target": target() }),
    )
}

#[test]
fn the_plan_follows_the_table() {
    assert!(matches!(
        plan(&request(click(1)), false),
        Action::AtTarget {
            gesture: Gesture::Click {
                count: 1,
                modifiers: 2,
                ..
            },
            ..
        }
    ));
    assert_eq!(plan(&request(click(0)), false), Action::Skip);
    let hover = event(json!({ "type": "hover", "target": target() }));
    assert_eq!(plan(&request(hover.clone()), true), Action::Skip);
    assert!(matches!(
        plan(&request(hover), false),
        Action::AtTarget {
            gesture: Gesture::Hover,
            ..
        }
    ));
    let scroll = event(json!({ "type": "scroll", "x": 0, "y": 640, "maxX": 0, "maxY": 2400 }));
    assert_eq!(
        plan(&request(scroll), false),
        Action::Scroll {
            x: 0.0,
            y: 640.0,
            max_x: 0.0,
            max_y: 2400.0
        }
    );
    let dead = event(
        json!({ "type": "keydown", "key": "Dead", "code": "Quote", "keyCode": 0, "location": 0, "repeat": false, "modifiers": 0 }),
    );
    assert_eq!(plan(&request(dead), false), Action::Skip);
    let back = event(
        json!({ "type": "click", "button": "back", "detail": 1, "modifiers": 0, "target": target() }),
    );
    assert_eq!(plan(&request(back), false), Action::Unsupported);
    let wheel = event(
        json!({ "type": "wheel", "deltaX": 0, "deltaY": 120, "deltaMode": 0, "modifiers": 0, "target": target() }),
    );
    assert_eq!(plan(&request(wheel), false), Action::Unsupported);
    let mut in_frame = request(click(1));
    in_frame.top = false;
    assert_eq!(plan(&in_frame, false), Action::Unsupported);
}

#[test]
fn a_click_is_press_and_release_at_the_point() {
    let gesture = Gesture::Click {
        button: rb_engine::Button::Left,
        count: 2,
        modifiers: 0,
    };
    let input = gesture_input(&gesture, 10.0, 20.0);
    assert_eq!(input.len(), 2);
    assert!(matches!(
        input[0],
        InputEvent::Mouse {
            phase: Phase::Down,
            clicks: 2,
            ..
        }
    ));
    assert!(matches!(
        input[1],
        InputEvent::Mouse {
            phase: Phase::Up,
            ..
        }
    ));
}

async fn setup() -> (Arc<FakeSidecar>, Arc<WebkitEngine>, WebkitReplay) {
    let fake = FakeSidecar::new();
    let webkit = WebkitEngine::new(Arc::new(fake.clone()), Handle::current(), "");
    webkit
        .create_view(&sample_spec(EngineKind::Webkit, 3))
        .await
        .expect("opens");
    let replay = WebkitReplay::new(&webkit);
    (fake, webkit, replay)
}

#[tokio::test]
async fn a_click_resolves_its_target_and_clicks_there_after_the_marker() {
    let (fake, _webkit, replay) = setup().await;
    let mut click = request(click(1));
    click.block_submit = true;
    assert_eq!(replay.dispatch(click).await, ReplayOutcome::Done);
    let resolve = &fake.calls_of(rpc::AGENT)[0];
    assert_eq!(resolve["command"]["cmd"], "resolve");
    assert_eq!(resolve["command"]["target"], target());
    let input = &fake.calls_of(rpc::INPUT)[0];
    assert_eq!(input["before"], json!({ "cmd": "replay", "ms": 300 }));
    assert_eq!(input["blockSubmitMs"], 1000);
    assert_eq!(input["events"][0]["x"], 10.0);
    assert_eq!(input["events"][0]["y"], 20.0);
    assert_eq!(input["events"][0]["modifiers"], 2);
}

#[tokio::test]
async fn a_missing_or_hidden_target_desyncs() {
    let (fake, _webkit, replay) = setup().await;
    fake.answer_agent(json!({ "status": "not_found" }));
    assert_eq!(
        replay.dispatch(request(click(1))).await,
        ReplayOutcome::NotFound
    );
    fake.answer_agent(json!({ "status": "not_visible" }));
    assert_eq!(
        replay.dispatch(request(click(1))).await,
        ReplayOutcome::NotVisible
    );
    assert!(fake.calls_of(rpc::INPUT).is_empty(), "nothing was clicked");
}

#[tokio::test]
async fn keys_text_and_scroll_need_no_point_or_a_field() {
    let (fake, _webkit, replay) = setup().await;
    let key = event(
        json!({ "type": "keydown", "key": "Enter", "code": "Enter", "keyCode": 13, "location": 0, "repeat": false, "modifiers": 0 }),
    );
    assert_eq!(replay.dispatch(request(key)).await, ReplayOutcome::Done);
    let text = event(
        json!({ "type": "input", "inputType": "insertText", "data": "a", "value": "a", "target": target() }),
    );
    assert_eq!(replay.dispatch(request(text)).await, ReplayOutcome::Done);
    let scroll = event(json!({ "type": "scroll", "x": 0, "y": 640, "maxX": 0, "maxY": 2400 }));
    assert_eq!(replay.dispatch(request(scroll)).await, ReplayOutcome::Done);
    let inputs = fake.calls_of(rpc::INPUT);
    assert_eq!(inputs[0]["events"][0]["type"], "key");
    assert_eq!(inputs[0]["events"][0]["key"], "Enter");
    assert_eq!(
        inputs[1]["events"][0],
        json!({ "type": "text", "text": "a" })
    );
    assert_eq!(fake.calls_of(rpc::SCROLL)[0]["maxY"], 2400.0);
}

#[tokio::test]
async fn a_view_the_engine_does_not_have_is_skipped() {
    let (fake, _webkit, replay) = setup().await;
    let mut elsewhere = request(click(1));
    elsewhere.target = ViewId(42);
    assert_eq!(replay.dispatch(elsewhere).await, ReplayOutcome::Unsupported);
    assert!(fake.calls_of(rpc::AGENT).is_empty());
}
