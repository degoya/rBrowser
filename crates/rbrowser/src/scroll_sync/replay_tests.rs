use std::sync::Arc;

use rb_config::ScrollMode;
use rb_engine::ViewId;
use rb_sync::{Replay, ReplayOutcome, ReplayRequest, SyncEvent, marks_desync};
use serde_json::{Value, json};

use super::*;
use crate::replay::CdpReplay;
use crate::scroll_sync::fake::ScrollPort;

const LEADER: ViewId = ViewId(1);
const VIEW: ViewId = ViewId(2);

fn event(fields: Value) -> SyncEvent {
    serde_json::from_value(fields).expect("an event")
}

fn scroll() -> SyncEvent {
    event(json!({
        "type": "scroll", "x": 0.0, "y": 2652.0, "maxX": 0.0, "maxY": 4632.0,
        "anchor": { "element": { "kind": "css", "path": "#s5" }, "offset": { "x": 0.0, "y": 0.25 } }
    }))
}

fn sidebar_scroll() -> SyncEvent {
    event(json!({
        "type": "scroll", "x": 0.0, "y": 100.0, "maxX": 0.0, "maxY": 752.0,
        "container": { "element": { "kind": "css", "path": "#sidebar" }, "offset": { "x": 0.5, "y": 0.5 } }
    }))
}

fn wheel(delta_mode: u32, delta_y: f64) -> SyncEvent {
    event(json!({
        "type": "wheel", "deltaX": 0.0, "deltaY": delta_y, "deltaMode": delta_mode,
        "modifiers": 8,
        "target": { "element": { "kind": "css", "path": "main" }, "offset": { "x": 0.5, "y": 0.5 } }
    }))
}

fn point(x: f64, y: f64, viewport: Value) -> Value {
    json!({ "resolved": { "status": "ok", "x": x, "y": y, "scrolled": false }, "viewport": viewport })
}

#[tokio::test]
async fn anchor_mode_applies_the_scroll_in_the_agents_world_in_one_round_trip() {
    let port = ScrollPort::new(ScrollMode::Anchor, json!({ "status": "done" }));
    assert_eq!(replay(&port, VIEW, &scroll()).await, Some(Ok(true)));
    let worlds = port.worlds();
    assert_eq!(worlds.len(), 1);
    let (view, source) = &worlds[0];
    assert_eq!(*view, VIEW);
    assert!(source.contains(r#""cmd":"scroll""#), "{source}");
    assert!(source.contains(r##""path":"#s5""##), "{source}");
    assert!(!source.contains(r#""cmd":"resolve""#));
    assert!(port.sends().is_empty(), "no input: the position is set");
}

#[tokio::test]
async fn anchor_mode_leaves_the_wheel_to_the_scroll() {
    let port = ScrollPort::new(ScrollMode::Anchor, json!({ "status": "done" }));
    assert_eq!(replay(&port, VIEW, &wheel(0, 120.0)).await, Some(Ok(false)));
    assert!(port.worlds().is_empty());
}

#[tokio::test]
async fn wheel_mode_turns_the_wheel_over_the_same_element() {
    let port = ScrollPort::new(
        ScrollMode::Wheel,
        point(195.0, 422.0, json!([0.0, 0.0, 1.0])),
    );
    assert_eq!(replay(&port, VIEW, &scroll()).await, Some(Ok(false)));
    assert!(port.worlds().is_empty(), "the scroll is left to the wheel");
    assert_eq!(replay(&port, VIEW, &wheel(1, 3.0)).await, Some(Ok(true)));
    let source = &port.worlds()[0].1;
    let marker = source.find(r#""cmd":"replay""#).expect("the echo marker");
    let point_at = source.find(r#""cmd":"point""#).expect("the point");
    assert!(marker < point_at, "the marker comes first: {source}");
    let sends = port.sends();
    assert_eq!(sends.len(), 1);
    let (view, method, params) = &sends[0];
    assert_eq!((*view, *method), (VIEW, "Input.dispatchMouseEvent"));
    assert_eq!(params["type"], "mouseWheel");
    assert_eq!(
        (params["x"].as_f64(), params["y"].as_f64()),
        (Some(195.0), Some(422.0))
    );
    assert_eq!(params["deltaY"].as_f64(), Some(3.0 * LINE_PX));
    assert_eq!(params["modifiers"], 8);
}

#[tokio::test]
async fn the_wheel_point_follows_a_pinch_zoom() {
    let port = ScrollPort::new(
        ScrollMode::Wheel,
        point(100.0, 300.0, json!([0.0, 100.0, 2.0])),
    );
    replay(&port, VIEW, &wheel(0, 50.0)).await;
    let params = &port.sends()[0].2;
    assert_eq!(
        (params["x"].as_f64(), params["y"].as_f64()),
        (Some(200.0), Some(400.0))
    );
}

#[test]
fn wheel_deltas_are_pixels_bounded_and_finite() {
    let SyncEvent::Wheel(mut data) = wheel(2, 1.0) else {
        panic!("a wheel");
    };
    assert_eq!(
        wheel_params(1.0, 2.0, &data)["deltaY"].as_f64(),
        Some(PAGE_PX)
    );
    data.delta_y = 1e12;
    data.delta_x = f64::NAN;
    let params = wheel_params(1.0, 2.0, &data);
    assert_eq!(params["deltaY"].as_f64(), Some(MAX_WHEEL_PX));
    assert_eq!(params["deltaX"].as_f64(), Some(0.0));
}

#[tokio::test]
async fn other_events_are_not_scroll_syncs() {
    let port = ScrollPort::new(ScrollMode::Anchor, json!({ "status": "done" }));
    let click = event(
        json!({ "type": "click", "button": "left", "detail": 1, "modifiers": 0,
        "target": { "element": { "kind": "css", "path": "#save" }, "offset": { "x": 0.5, "y": 0.5 } } }),
    );
    assert_eq!(replay(&port, VIEW, &click).await, None);
}

#[tokio::test]
async fn a_missing_container_is_not_found_and_never_desyncs() {
    let port = ScrollPort::new(ScrollMode::Anchor, json!({ "status": "not_found" }));
    let event = sidebar_scroll();
    assert_eq!(
        replay(&port, VIEW, &event).await,
        Some(Err(ReplayOutcome::NotFound))
    );
    assert!(!marks_desync(&event));
    assert!(port.worlds()[0].1.contains(r##""path":"#sidebar""##));
}

#[test]
fn the_agents_answers_read_without_quoting_them() {
    assert_eq!(scroll_outcome(&json!({ "status": "done" })), Ok(()));
    assert_eq!(
        scroll_outcome(&json!({ "status": "not_visible" })),
        Err(ReplayOutcome::NotVisible)
    );
    for answer in [
        json!({ "error": "no agent" }),
        json!({ "status": "error", "message": "#secret-selector" }),
        json!({ "status": "ok", "x": 1.0, "y": 2.0, "scrolled": false }),
        json!("garbage #secret-selector"),
    ] {
        let Err(ReplayOutcome::Failed(message)) = scroll_outcome(&answer) else {
            panic!("{answer} is a failure");
        };
        assert!(!message.contains("secret"), "{message}");
    }
}

#[test]
fn a_required_target_resolves_before_the_scroll() {
    let SyncEvent::Scroll(data) = scroll() else {
        panic!("a scroll");
    };
    let target = serde_json::from_value(
        json!({ "element": { "kind": "css", "path": "#s5" }, "offset": { "x": 0.0, "y": 0.0 } }),
    )
    .expect("a target");
    let source = scroll_expression(&data, Some(&target)).expect("an expression");
    let resolve = source.find(r#""cmd":"resolve""#).expect("the check");
    let scroll = source.find(r#""cmd":"scroll""#).expect("the scroll");
    assert!(resolve < scroll);
    // A selector cannot break out of the expression: it is JSON.
    let mut hostile = data.clone();
    hostile.anchor = serde_json::from_value(
        json!({ "element": { "kind": "css", "path": "'});alert(1);({'" }, "offset": { "x": 0.0, "y": 0.0 } }),
    )
    .expect("a target");
    let source = scroll_expression(&hostile, None).expect("an expression");
    assert!(source.contains(r#""path":"'});alert(1);({'""#));
}

#[tokio::test]
async fn the_cdp_replay_hands_scroll_to_the_scroll_sync() {
    let port = Arc::new(ScrollPort::new(
        ScrollMode::Anchor,
        json!({ "status": "done" }),
    ));
    let replay = CdpReplay::new(port.clone());
    let request = ReplayRequest {
        source: LEADER,
        target: VIEW,
        top: true,
        frame: None,
        event: scroll().into(),
        block_submit: true,
    };
    assert_eq!(replay.dispatch(request.clone()).await, ReplayOutcome::Done);
    assert_eq!(port.worlds().len(), 1);
    // Never into the leader; an iframe's scroll is not synced yet.
    let mut into_leader = request.clone();
    into_leader.target = LEADER;
    assert_eq!(
        replay.dispatch(into_leader).await,
        ReplayOutcome::Unsupported
    );
    let mut in_frame = request;
    in_frame.top = false;
    assert_eq!(replay.dispatch(in_frame).await, ReplayOutcome::Unsupported);
    assert_eq!(port.worlds().len(), 1);
}
