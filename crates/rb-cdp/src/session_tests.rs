use serde_json::{Value, json};

use super::*;
use crate::target_filter::HiddenTargets;

fn filter() -> HiddenTargets {
    HiddenTargets::new(&["devtools:", "rbrowser:"], &[])
}

fn no_scale(_: &str) -> Option<f64> {
    None
}

fn call(session: &ClientSession, message: Value) -> Call {
    match session.read_message(&message.to_string(), &no_scale) {
        FromClient::Call(call) => call,
        other => panic!("no call: {other:?}"),
    }
}

fn answer(session: &ClientSession, message: Value) -> Value {
    match session.read_message(&message.to_string(), &no_scale) {
        FromClient::Answer(answer) => answer,
        other => panic!("no answer: {other:?}"),
    }
}

/// A browser session with the child `C1` on target `V1`.
fn with_child() -> ClientSession {
    let mut session = ClientSession::browser("ROOT".to_owned());
    let attach = call(
        &session,
        json!({"id": 1, "method": "Target.attachToTarget", "params": {"targetId": "V1", "flatten": true}}),
    );
    session.answer(&attach, Ok(json!({"sessionId": "C1"})), &filter());
    session
}

#[test]
fn a_call_without_session_goes_to_the_root_with_the_clients_id() {
    let session = ClientSession::browser("ROOT".to_owned());
    let call = call(&session, json!({"id": 7, "method": "Browser.getVersion"}));
    assert_eq!(call.id, json!(7));
    assert_eq!(call.session, "ROOT");
    assert_eq!(call.child, None);
    assert_eq!(call.params, json!({}));
    assert_eq!(call.target_check, None);
    assert_eq!(session.root(), "ROOT");
}

#[test]
fn malformed_messages_are_dropped_or_answered() {
    let session = ClientSession::browser("ROOT".to_owned());
    for text in [
        "not json",
        "[1]",
        r#"{"method": "Page.enable"}"#,
        r#"{"id": -1, "method": "x"}"#,
    ] {
        assert_eq!(
            session.read_message(text, &no_scale),
            FromClient::Drop,
            "{text}"
        );
    }
    let no_method = answer(&session, json!({"id": 1}));
    assert_eq!(no_method["error"]["code"], json!(-32600));
    let bad_params = answer(
        &session,
        json!({"id": 2, "method": "Page.navigate", "params": [1]}),
    );
    assert_eq!(bad_params["error"]["code"], json!(-32600));
}

#[test]
fn a_session_the_client_does_not_own_is_unknown() {
    let session = with_child();
    let foreign = answer(
        &session,
        json!({"id": 3, "method": "Page.enable", "sessionId": "OTHER"}),
    );
    assert_eq!(foreign["error"]["code"], json!(-32001));
    assert_eq!(foreign["sessionId"], json!("OTHER"));
    let root = answer(
        &session,
        json!({"id": 4, "method": "Page.enable", "sessionId": "ROOT"}),
    );
    assert_eq!(root["error"]["code"], json!(-32001));
    let own = call(
        &session,
        json!({"id": 5, "method": "Page.enable", "sessionId": "C1"}),
    );
    assert_eq!(own.session, "C1");
    assert_eq!(own.child.as_deref(), Some("C1"));
}

#[test]
fn device_metrics_are_answered_empty_and_never_sent() {
    let session = with_child();
    let owned = answer(
        &session,
        json!({"id": 9, "sessionId": "C1", "method": "Emulation.setDeviceMetricsOverride", "params": {"width": 1, "height": 1, "deviceScaleFactor": 1, "mobile": false}}),
    );
    assert_eq!(owned, json!({"id": 9, "result": {}, "sessionId": "C1"}));
}

#[test]
fn refused_methods_are_answered_with_the_reason() {
    let session = ClientSession::browser("ROOT".to_owned());
    for method in [
        "Browser.close",
        "Target.createTarget",
        "Target.attachToBrowserTarget",
    ] {
        let refused = answer(&session, json!({"id": 1, "method": method}));
        assert_eq!(refused["error"]["code"], json!(-32000), "{method}");
        assert!(
            refused["error"]["message"]
                .as_str()
                .is_some_and(|m| !m.is_empty())
        );
    }
}

#[test]
fn a_target_id_is_checked_before_it_is_sent() {
    let session = ClientSession::browser("ROOT".to_owned());
    let attach = call(
        &session,
        json!({"id": 1, "method": "Target.attachToTarget", "params": {"targetId": "X", "flatten": true}}),
    );
    assert_eq!(attach.target_check.as_deref(), Some("X"));
    let refused = ClientSession::no_target(&attach);
    assert_eq!(refused["id"], json!(1));
    assert_eq!(refused["error"]["code"], json!(-32602));
    assert_eq!(
        refused["error"]["message"],
        json!("No target with given id found")
    );
}

#[test]
fn a_target_seen_hidden_is_refused_at_once() {
    let mut session = ClientSession::browser("ROOT".to_owned());
    let created = json!({"targetInfo": {"targetId": "SHELL", "type": "page", "url": "rbrowser://ui/index.html"}});
    assert_eq!(
        session.read_event("Target.targetCreated", &created, Some("ROOT"), &filter()),
        ToClient::Skip
    );
    let guessed = answer(
        &session,
        json!({"id": 2, "method": "Target.attachToTarget", "params": {"targetId": "SHELL", "flatten": true}}),
    );
    assert_eq!(guessed["error"]["code"], json!(-32602));
}

#[test]
fn only_flat_sessions_are_attached() {
    let session = ClientSession::browser("ROOT".to_owned());
    for params in [
        json!({"targetId": "V1"}),
        json!({"targetId": "V1", "flatten": false}),
    ] {
        let refused = answer(
            &session,
            json!({"id": 1, "method": "Target.attachToTarget", "params": params}),
        );
        assert_eq!(refused["error"]["code"], json!(-32000));
    }
}

#[test]
fn only_own_children_are_detached_and_only_by_session() {
    let session = with_child();
    let own = call(
        &session,
        json!({"id": 1, "method": "Target.detachFromTarget", "params": {"sessionId": "C1"}}),
    );
    assert_eq!(own.session, "ROOT");
    let foreign = answer(
        &session,
        json!({"id": 2, "method": "Target.detachFromTarget", "params": {"sessionId": "OTHER"}}),
    );
    assert_eq!(foreign["error"]["code"], json!(-32001));
    let root = answer(
        &session,
        json!({"id": 3, "method": "Target.detachFromTarget", "params": {"sessionId": "ROOT"}}),
    );
    assert_eq!(root["error"]["code"], json!(-32001));
    let by_target = answer(
        &session,
        json!({"id": 4, "method": "Target.detachFromTarget", "params": {"targetId": "V1"}}),
    );
    assert_eq!(by_target["error"]["code"], json!(-32602));
}

#[test]
fn input_on_a_child_is_scaled_by_its_targets_scale() {
    let session = with_child();
    let scale = |target: &str| (target == "V1").then_some(0.5);
    let message = json!({"id": 1, "sessionId": "C1", "method": "Input.dispatchMouseEvent", "params": {"type": "mousePressed", "x": 100, "y": 40}});
    let FromClient::Call(call) = session.read_message(&message.to_string(), &scale) else {
        panic!("no call");
    };
    assert_eq!(call.params["x"], json!(50.0));
    assert_eq!(call.params["y"], json!(20.0));
    // On the browser session there is no page and nothing is scaled.
    let root = json!({"id": 2, "method": "Input.dispatchMouseEvent", "params": {"type": "mouseMoved", "x": 100, "y": 40}});
    let FromClient::Call(call) = session.read_message(&root.to_string(), &scale) else {
        panic!("no call");
    };
    assert_eq!(call.params["x"], json!(100));
}

#[test]
fn get_targets_answers_only_visible_targets() {
    let mut session = ClientSession::browser("ROOT".to_owned());
    let asked = call(&session, json!({"id": 3, "method": "Target.getTargets"}));
    let reply = json!({"targetInfos": [
        {"targetId": "SHELL", "type": "page", "url": "rbrowser://ui/index.html"},
        {"targetId": "V1", "type": "page", "url": "https://a.test/"},
        {"targetId": "DT", "type": "page", "url": "devtools://devtools/bundled/inspector.html"},
        {"targetId": "SET", "type": "page", "url": "rbrowser://ui/settings.html"},
    ]});
    let answered = session.answer(&asked, Ok(reply), &filter());
    assert_eq!(
        answered,
        json!({"id": 3, "result": {"targetInfos": [{"targetId": "V1", "type": "page", "url": "https://a.test/"}]}})
    );
    // Those it never saw are refused at once from now on.
    let guessed = answer(
        &session,
        json!({"id": 4, "method": "Target.activateTarget", "params": {"targetId": "DT"}}),
    );
    assert_eq!(guessed["error"]["code"], json!(-32602));
}

#[test]
fn an_attach_answer_makes_the_session_the_clients() {
    let session = with_child();
    assert_eq!(session.children(), vec!["C1".to_owned()]);
    assert_eq!(session.touched(), vec!["V1".to_owned()]);
}

#[test]
fn errors_keep_their_code_and_a_zero_code_becomes_a_server_error() {
    let mut session = with_child();
    let asked = call(
        &session,
        json!({"id": 6, "sessionId": "C1", "method": "Page.navigate", "params": {"url": "x"}}),
    );
    let failed = session.answer(
        &asked,
        Err(CallError {
            code: -32602,
            message: "Invalid URL".to_owned(),
        }),
        &filter(),
    );
    assert_eq!(
        failed,
        json!({"id": 6, "error": {"code": -32602, "message": "Invalid URL"}, "sessionId": "C1"})
    );
    let timed_out = session.answer(
        &asked,
        Err(CallError {
            code: 0,
            message: "late".to_owned(),
        }),
        &filter(),
    );
    assert_eq!(timed_out["error"]["code"], json!(-32000));
}

#[test]
fn auto_attached_hidden_targets_are_detached_silently() {
    let mut session = ClientSession::browser("ROOT".to_owned());
    let attached = json!({"sessionId": "H1", "targetInfo": {"targetId": "SHELL", "type": "page", "url": "rbrowser://ui/index.html"}, "waitingForDebugger": true});
    assert_eq!(
        session.read_event(
            "Target.attachedToTarget",
            &attached,
            Some("ROOT"),
            &filter()
        ),
        ToClient::DetachHidden {
            on: "ROOT".to_owned(),
            session: "H1".to_owned()
        }
    );
    assert!(session.children().is_empty());
    let detached = json!({"sessionId": "H1", "targetId": "SHELL"});
    assert_eq!(
        session.read_event(
            "Target.detachedFromTarget",
            &detached,
            Some("ROOT"),
            &filter()
        ),
        ToClient::Skip
    );
    let destroyed = json!({"targetId": "SHELL"});
    assert_eq!(
        session.read_event(
            "Target.targetDestroyed",
            &destroyed,
            Some("ROOT"),
            &filter()
        ),
        ToClient::Skip
    );
}

#[test]
fn visible_auto_attached_targets_become_children_and_their_events_flow() {
    let mut session = ClientSession::browser("ROOT".to_owned());
    let attached = json!({"sessionId": "C2", "targetInfo": {"targetId": "V2", "type": "page", "url": "https://b.test/"}, "waitingForDebugger": true});
    assert_eq!(
        session.read_event(
            "Target.attachedToTarget",
            &attached,
            Some("ROOT"),
            &filter()
        ),
        ToClient::Forward(json!({"method": "Target.attachedToTarget", "params": attached}))
    );
    let loaded = json!({"timestamp": 1.0});
    assert_eq!(
        session.read_event("Page.loadEventFired", &loaded, Some("C2"), &filter()),
        ToClient::Forward(
            json!({"method": "Page.loadEventFired", "params": loaded, "sessionId": "C2"})
        )
    );
    let detached = json!({"sessionId": "C2", "targetId": "V2"});
    assert!(matches!(
        session.read_event(
            "Target.detachedFromTarget",
            &detached,
            Some("ROOT"),
            &filter()
        ),
        ToClient::Forward(_)
    ));
    assert!(session.children().is_empty());
    assert_eq!(session.touched(), vec!["V2".to_owned()]);
}

#[test]
fn events_of_other_sessions_and_the_internal_client_are_not_the_clients() {
    let mut session = with_child();
    let event = json!({"timestamp": 1.0});
    assert_eq!(
        session.read_event("Page.loadEventFired", &event, Some("OTHER"), &filter()),
        ToClient::Skip
    );
    assert_eq!(
        session.read_event("Page.loadEventFired", &event, None, &filter()),
        ToClient::Skip
    );
    let others_detach = json!({"sessionId": "OTHER"});
    assert_eq!(
        session.read_event("Target.detachedFromTarget", &others_detach, None, &filter()),
        ToClient::Skip
    );
    let own_detach = json!({"sessionId": "ROOT"});
    assert_eq!(
        session.read_event("Target.detachedFromTarget", &own_detach, None, &filter()),
        ToClient::Gone
    );
}

#[test]
fn info_changes_hide_a_target_that_turns_internal() {
    let mut session = ClientSession::browser("ROOT".to_owned());
    let visible =
        json!({"targetInfo": {"targetId": "V3", "type": "page", "url": "https://c.test/"}});
    assert!(matches!(
        session.read_event(
            "Target.targetInfoChanged",
            &visible,
            Some("ROOT"),
            &filter()
        ),
        ToClient::Forward(_)
    ));
    let internal =
        json!({"targetInfo": {"targetId": "V3", "type": "page", "url": "devtools://devtools/x"}});
    assert_eq!(
        session.read_event(
            "Target.targetInfoChanged",
            &internal,
            Some("ROOT"),
            &filter()
        ),
        ToClient::Skip
    );
}

#[test]
fn a_page_session_sends_to_its_page_and_scales_by_its_target() {
    let session = ClientSession::page("P".to_owned(), "V1".to_owned());
    let scale = |target: &str| (target == "V1").then_some(0.5);
    let message = json!({"id": 1, "method": "Input.dispatchMouseEvent", "params": {"type": "mouseMoved", "x": 10, "y": 10}});
    let FromClient::Call(call) = session.read_message(&message.to_string(), &scale) else {
        panic!("no call");
    };
    assert_eq!(call.session, "P");
    assert_eq!(call.params["x"], json!(5.0));
    assert_eq!(session.touched(), vec!["V1".to_owned()]);
}
