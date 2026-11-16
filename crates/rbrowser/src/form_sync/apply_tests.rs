use std::sync::Arc;

use rb_engine::ViewId;
use rb_sync::{Replay, ReplayOutcome, ReplayRequest, SyncEvent};
use serde_json::{Value, json};

use crate::cdp::lock;
use crate::replay::CdpReplay;
use crate::replay::fake::FakePort;

const LEADER: ViewId = ViewId(1);
const VIEW: ViewId = ViewId(2);

fn target(path: &str) -> Value {
    json!({ "element": { "kind": "css", "path": path }, "offset": { "x": 0.5, "y": 0.5 } })
}

fn event(fields: Value) -> SyncEvent {
    serde_json::from_value(fields).expect("an event")
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
async fn a_select_change_is_one_apply_in_the_agent_s_world() {
    let (port, replay) = setup();
    let change = event(json!({ "type": "change", "value": "de", "selected": [1],
                               "target": target("#lang") }));
    assert_eq!(
        replay.dispatch(request(change, true)).await,
        ReplayOutcome::Done
    );
    let sent = port.sent_to(VIEW);
    assert_eq!(sent.len(), 1, "no input, one evaluation");
    let expression = sent[0].params.as_str().expect("an expression");
    assert!(expression.contains(r#""cmd":"apply""#));
    assert!(expression.contains(r#""selected":[1]"#));
    assert!(
        expression.contains("addEventListener('submit'"),
        "leader-only submit holds"
    );
}

#[tokio::test]
async fn focus_is_one_focus_command() {
    let (port, replay) = setup();
    let focus = event(json!({ "type": "focus", "target": target("#q") }));
    assert_eq!(
        replay.dispatch(request(focus, false)).await,
        ReplayOutcome::Done
    );
    let sent = port.sent_to(VIEW);
    assert_eq!(sent.len(), 1);
    assert!(
        sent[0]
            .params
            .as_str()
            .is_some_and(|expression| expression.contains(r#""cmd":"focus""#))
    );
}

#[tokio::test]
async fn a_file_input_is_skipped_without_a_command_or_a_desync() {
    let (port, replay) = setup();
    let file = event(json!({ "type": "change", "target": target("#avatar") }));
    assert_eq!(
        replay.dispatch(request(file, false)).await,
        ReplayOutcome::Done
    );
    assert!(port.sent_to(VIEW).is_empty());
}

#[tokio::test]
async fn a_control_the_view_lacks_is_a_miss() {
    let (port, replay) = setup();
    lock(&port.applied).insert(VIEW, json!({ "status": "not_found" }));
    let change = event(json!({ "type": "change", "checked": true, "value": "on",
                               "target": target("#terms") }));
    assert_eq!(
        replay.dispatch(request(change, false)).await,
        ReplayOutcome::NotFound
    );
}

#[tokio::test]
async fn a_password_travels_to_the_view_but_not_into_the_outcome() {
    let (port, replay) = setup();
    lock(&port.applied).insert(VIEW, json!({ "status": "error", "message": "hunter2" }));
    let change = event(
        json!({ "type": "change", "value": "hunter2", "sensitive": true,
                               "target": target("#secret") }),
    );
    let outcome = replay.dispatch(request(change, false)).await;
    assert!(
        port.sent_to(VIEW)[0]
            .params
            .as_str()
            .is_some_and(|expression| expression.contains("hunter2")),
        "the view needs the value"
    );
    assert!(matches!(&outcome, ReplayOutcome::Failed(reason) if !reason.contains("hunter2")));
    assert!(!format!("{outcome:?}").contains("hunter2"));
}

#[tokio::test]
async fn an_iframe_s_change_is_not_replayed_yet() {
    let (port, replay) = setup();
    let change = event(json!({ "type": "change", "value": "de", "target": target("#lang") }));
    let mut request = request(change, false);
    request.top = false;
    assert_eq!(replay.dispatch(request).await, ReplayOutcome::Unsupported);
    assert!(port.sent_to(VIEW).is_empty());
}
