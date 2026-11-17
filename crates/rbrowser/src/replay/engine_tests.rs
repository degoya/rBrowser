use std::sync::Arc;

use rb_engine::ViewId;
use rb_sync::{Replay, ReplayOutcome, ReplayRequest, SyncEvent};
use serde_json::{Value, json};

use super::*;
use crate::cdp::lock;
use crate::replay::fake::FakePort;

const LEADER: ViewId = ViewId(1);
const VIEW: ViewId = ViewId(2);

fn target(path: &str) -> Value {
    json!({ "element": { "kind": "css", "path": path }, "offset": { "x": 0.5, "y": 0.5 } })
}

fn event(fields: Value) -> SyncEvent {
    serde_json::from_value(fields).expect("an event")
}

fn click(detail: u32) -> SyncEvent {
    event(
        json!({ "type": "click", "button": "left", "detail": detail, "modifiers": 0,
                  "target": target("#save") }),
    )
}

fn request(event: SyncEvent, block_submit: bool) -> ReplayRequest {
    ReplayRequest {
        source: LEADER,
        target: VIEW,
        top: true,
        frame: None,
        event: event.into(),
        block_submit,
    }
}

fn setup() -> (Arc<FakePort>, CdpReplay) {
    let port = Arc::new(FakePort::default());
    let replay = CdpReplay::new(port.clone());
    (port, replay)
}

#[tokio::test]
async fn a_click_is_resolved_then_moved_pressed_and_released_at_its_point() {
    let (port, replay) = setup();
    assert_eq!(
        replay.dispatch(request(click(1), false)).await,
        ReplayOutcome::Done
    );
    assert_eq!(
        port.kinds(VIEW),
        [
            "Runtime.evaluate",
            "Input.dispatchMouseEvent:mouseMoved",
            "Input.dispatchMouseEvent:mousePressed",
            "Input.dispatchMouseEvent:mouseReleased",
        ]
    );
    let sent = port.sent_to(VIEW);
    let pressed = &sent[2].params;
    assert_eq!(
        (pressed["x"].as_f64(), pressed["y"].as_f64()),
        (Some(100.0), Some(50.0))
    );
    assert_eq!(pressed["button"], "left");
    assert_eq!(pressed["clickCount"], 1);
    assert_eq!(replay.latency().samples, 1);
}

#[tokio::test]
async fn a_double_click_is_two_clicks_counting_one_and_two() {
    let (port, replay) = setup();
    for detail in [1, 2] {
        replay.dispatch(request(click(detail), false)).await;
    }
    let dblclick = event(json!({ "type": "dblclick", "button": "left", "detail": 2,
                                 "modifiers": 0, "target": target("#save") }));
    assert_eq!(
        replay.dispatch(request(dblclick, false)).await,
        ReplayOutcome::Done
    );
    let counts: Vec<Value> = port
        .sent_to(VIEW)
        .iter()
        .filter(|sent| sent.params["type"] == "mousePressed")
        .map(|sent| sent.params["clickCount"].clone())
        .collect();
    assert_eq!(counts, [json!(1), json!(2)]);
}

#[tokio::test]
async fn the_echo_marker_is_set_before_any_input() {
    let (port, replay) = setup();
    let key = event(
        json!({ "type": "keydown", "key": "Enter", "code": "Enter", "keyCode": 13,
                            "location": 0, "repeat": false, "modifiers": 0 }),
    );
    for event in [click(1), key] {
        replay.dispatch(request(event, false)).await;
    }
    let sent = port.sent_to(VIEW);
    let prologues: Vec<usize> = (0..sent.len())
        .filter(|at| sent[*at].method == "Runtime.evaluate")
        .collect();
    assert_eq!(
        prologues,
        [0, 4],
        "one prologue in front of each event's input"
    );
    for at in prologues {
        let source = sent[at].params.as_str().expect("an expression");
        assert!(source.contains(r#""cmd":"replay""#));
    }
    assert_eq!(sent[5].params["type"], "keyDown");
    assert_eq!(sent[5].params["text"], "\r");
}

#[tokio::test]
async fn nothing_is_replayed_into_the_leader() {
    let (port, replay) = setup();
    let mut into_leader = request(click(1), false);
    into_leader.target = LEADER;
    assert_eq!(
        replay.dispatch(into_leader).await,
        ReplayOutcome::Unsupported
    );
    assert!(port.sent().is_empty());
}

#[tokio::test]
async fn a_missing_or_hidden_target_is_reported_for_the_desync() {
    let (port, replay) = setup();
    port.resolve_to(VIEW, json!({ "status": "not_found" }));
    assert_eq!(
        replay.dispatch(request(click(1), false)).await,
        ReplayOutcome::NotFound
    );
    port.resolve_to(VIEW, json!({ "status": "not_visible" }));
    assert_eq!(
        replay.dispatch(request(click(1), false)).await,
        ReplayOutcome::NotVisible
    );
    port.resolve_to(
        VIEW,
        json!({ "status": "error", "message": "invalid target" }),
    );
    assert!(matches!(
        replay.dispatch(request(click(1), false)).await,
        ReplayOutcome::Failed(_)
    ));
    // No input went to a view that missed.
    assert!(
        port.kinds(VIEW)
            .iter()
            .all(|kind| kind == "Runtime.evaluate")
    );
}

#[tokio::test]
async fn block_submit_sets_the_submit_guard_in_the_prologue() {
    let (port, replay) = setup();
    replay.dispatch(request(click(1), true)).await;
    replay.dispatch(request(click(1), false)).await;
    let prologues: Vec<String> = port
        .sent_to(VIEW)
        .iter()
        .filter(|sent| sent.method == "Runtime.evaluate")
        .filter_map(|sent| sent.params.as_str().map(str::to_owned))
        .collect();
    assert!(prologues[0].contains("addEventListener('submit'"));
    assert!(!prologues[1].contains("submit"));
}

#[tokio::test]
async fn typed_text_goes_in_after_the_field_resolved_and_a_password_stays_out_of_errors() {
    let (port, replay) = setup();
    let typed = event(
        json!({ "type": "input", "inputType": "insertText", "data": "s3cret",
                              "value": "s3cret", "sensitive": true, "target": target("#pw") }),
    );
    assert_eq!(
        replay.dispatch(request(typed.clone(), false)).await,
        ReplayOutcome::Done
    );
    assert_eq!(port.kinds(VIEW), ["Runtime.evaluate", "Input.insertText"]);
    assert_eq!(port.sent_to(VIEW)[1].params["text"], "s3cret");
    // A view whose CDP fails: the failure names the method, never the value.
    let mut failing = request(typed, false);
    failing.target = ViewId(99);
    match replay.dispatch(failing).await {
        ReplayOutcome::Failed(message) => {
            assert!(message.contains("Input.insertText"));
            assert!(!message.contains("s3cret"));
        }
        other => panic!("a failure, not {other:?}"),
    }
}

#[tokio::test]
async fn a_touch_view_gets_a_tap() {
    let (port, replay) = setup();
    lock(&port.touch).push(VIEW);
    assert_eq!(
        replay.dispatch(request(click(1), false)).await,
        ReplayOutcome::Done
    );
    assert_eq!(
        port.kinds(VIEW),
        [
            "Runtime.evaluate",
            "Input.dispatchTouchEvent:touchStart",
            "Input.dispatchTouchEvent:touchEnd",
        ]
    );
}

#[tokio::test]
async fn hover_moves_the_mouse_only() {
    let (port, replay) = setup();
    let hover = event(json!({ "type": "hover", "target": target("nav > a") }));
    assert_eq!(
        replay.dispatch(request(hover, false)).await,
        ReplayOutcome::Done
    );
    assert_eq!(
        port.kinds(VIEW),
        ["Runtime.evaluate", "Input.dispatchMouseEvent:mouseMoved"]
    );
}

#[tokio::test]
async fn a_pinch_zoomed_view_gets_the_point_in_its_visual_viewport() {
    let (port, replay) = setup();
    *lock(&port.viewport) = json!([20.0, 10.0, 2.0]);
    replay.dispatch(request(click(1), false)).await;
    let moved = &port.sent_to(VIEW)[1].params;
    // (100 - 20) * 2, (50 - 10) * 2.
    assert_eq!(
        (moved["x"].as_f64(), moved["y"].as_f64()),
        (Some(160.0), Some(80.0))
    );
}

#[tokio::test]
async fn the_context_menu_is_replayed_only_when_switched_on() {
    let menu = event(
        json!({ "type": "contextmenu", "button": "right", "detail": 0,
                             "modifiers": 0, "target": target("#row") }),
    );
    let (port, replay) = setup();
    assert_eq!(
        replay.dispatch(request(menu.clone(), false)).await,
        ReplayOutcome::Done
    );
    assert!(port.sent().is_empty());
    let port = Arc::new(FakePort {
        context_menu: true,
        ..FakePort::default()
    });
    let replay = CdpReplay::new(port.clone());
    replay.dispatch(request(menu, false)).await;
    assert_eq!(port.sent_to(VIEW)[2].params["button"], "right");
}

#[tokio::test]
async fn iframe_events_and_other_jobs_events_are_skipped_without_desync() {
    let (port, replay) = setup();
    let mut in_frame = request(click(1), false);
    in_frame.top = false;
    assert_eq!(replay.dispatch(in_frame).await, ReplayOutcome::Unsupported);
    let pointer = event(
        json!({ "type": "pointer", "phase": "down", "button": "left",
        "buttons": 1, "pointerType": "mouse", "modifiers": 0, "target": target("#save") }),
    );
    assert_eq!(
        replay.dispatch(request(pointer, false)).await,
        ReplayOutcome::Unsupported
    );
    assert!(port.sent().is_empty());
    assert_eq!(replay.latency().samples, 0);
}

#[tokio::test]
async fn a_view_that_is_gone_fails() {
    let (_port, replay) = setup();
    let mut gone = request(click(1), false);
    gone.target = ViewId(500);
    assert!(matches!(
        replay.dispatch(gone).await,
        ReplayOutcome::Failed(_)
    ));
}
