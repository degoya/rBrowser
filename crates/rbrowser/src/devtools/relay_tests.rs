use serde_json::json;

use super::*;

fn relay() -> Relay {
    Relay::new("PAGE".to_owned(), "T1".to_owned())
}

fn target_info(id: &str, kind: &str) -> Value {
    json!({ "targetId": id, "type": kind, "url": "https://a.test/" })
}

fn event(method: &str, params: Value, session: Option<&str>) -> CdpEvent {
    CdpEvent {
        method: method.to_owned(),
        params,
        session_id: session.map(str::to_owned),
    }
}

fn call(relay: &Relay, message: Value) -> Call {
    match relay.read_message(&message.to_string()) {
        FromFrontend::Call(call) => call,
        other => panic!("not a call: {other:?}"),
    }
}

fn answer(relay: &Relay, message: Value) -> Value {
    match relay.read_message(&message.to_string()) {
        FromFrontend::Answer(answer) => answer,
        other => panic!("not an answer: {other:?}"),
    }
}

#[test]
fn a_call_without_session_goes_to_the_page() {
    let relay = relay();
    let call = call(
        &relay,
        json!({ "id": 7, "method": "DOM.getDocument", "params": { "depth": -1 } }),
    );
    assert_eq!(
        call,
        Call {
            id: json!(7),
            method: "DOM.getDocument".to_owned(),
            params: json!({ "depth": -1 }),
            session: "PAGE".to_owned(),
            child: None,
        }
    );
    let missing_params = self::call(&relay, json!({ "id": 8, "method": "Runtime.enable" }));
    assert_eq!(missing_params.params, json!({}));
}

#[test]
fn the_answer_carries_the_frontends_id_and_session() {
    let relay = relay();
    let plain = call(&relay, json!({ "id": 3, "method": "Page.enable" }));
    assert_eq!(
        Relay::answer(&plain, Ok(json!({ "ok": 1 }))),
        json!({ "id": 3, "result": { "ok": 1 } })
    );
    let failed = Relay::answer(
        &plain,
        Err(EngineError::Cdp {
            method: "Page.enable".to_owned(),
            code: -32601,
            message: "'Page.enable' wasn't found".to_owned(),
        }),
    );
    assert_eq!(
        failed,
        json!({ "id": 3, "error": { "code": -32601, "message": "'Page.enable' wasn't found" } })
    );
    let closed = Relay::answer(&plain, Err(EngineError::Closed));
    assert_eq!(closed["error"]["code"], -32000);
}

#[test]
fn children_are_admitted_once_the_page_announced_them_and_until_they_detach() {
    let mut relay = relay();
    let unknown = answer(
        &relay,
        json!({ "id": 1, "method": "Runtime.enable", "sessionId": "CHILD" }),
    );
    assert_eq!(unknown["error"]["code"], -32001);
    assert_eq!(unknown["sessionId"], "CHILD");
    let attached = relay.read_event(&event(
        "Target.attachedToTarget",
        json!({ "sessionId": "CHILD", "targetInfo": { "targetId": "W1", "type": "worker", "url": "https://a.test/w.js" } }),
        Some("PAGE"),
    ));
    assert!(
        matches!(attached, ToFrontend::Forward(ref message) if message.get("sessionId").is_none())
    );
    let child_call = call(
        &relay,
        json!({ "id": 2, "method": "Runtime.enable", "sessionId": "CHILD" }),
    );
    assert_eq!(child_call.session, "CHILD");
    assert_eq!(
        Relay::answer(&child_call, Ok(json!({}))),
        json!({ "id": 2, "result": {}, "sessionId": "CHILD" })
    );
    let child_event =
        relay.read_event(&event("Runtime.consoleAPICalled", json!({}), Some("CHILD")));
    assert_eq!(
        child_event,
        ToFrontend::Forward(
            json!({ "method": "Runtime.consoleAPICalled", "params": {}, "sessionId": "CHILD" })
        )
    );
    relay.read_event(&event(
        "Target.detachedFromTarget",
        json!({ "sessionId": "CHILD" }),
        Some("PAGE"),
    ));
    let gone = answer(
        &relay,
        json!({ "id": 3, "method": "Runtime.enable", "sessionId": "CHILD" }),
    );
    assert_eq!(gone["error"]["code"], -32001);
}

#[test]
fn the_browser_and_other_targets_stay_out_of_reach() {
    let relay = relay();
    for (method, params) in [
        ("Target.getTargets", json!({})),
        (
            "Target.attachToTarget",
            json!({ "targetId": "SHELL", "flatten": true }),
        ),
        // Only a flat session: a non-flat one needs `Target.sendMessageToTarget`.
        ("Target.attachToTarget", json!({ "targetId": "T1" })),
        (
            "Target.setRemoteLocations",
            json!({ "locations": [{ "host": "localhost", "port": 9229 }] }),
        ),
        ("Target.attachToBrowserTarget", json!({})),
        ("Target.createTarget", json!({ "url": "about:blank" })),
        ("Target.closeTarget", json!({ "targetId": "SHELL" })),
        ("Target.getTargetInfo", json!({ "targetId": "SHELL" })),
        (
            "Target.detachFromTarget",
            json!({ "sessionId": "S-OF-THE-HOST" }),
        ),
        (
            "Target.exposeDevToolsProtocol",
            json!({ "targetId": "SHELL" }),
        ),
        ("Browser.close", json!({})),
        (
            "Browser.setDownloadBehavior",
            json!({ "behavior": "allow" }),
        ),
    ] {
        let refused = answer(
            &relay,
            json!({ "id": 9, "method": method, "params": params }),
        );
        assert_eq!(refused["error"]["message"], "Not allowed", "{method}");
    }
    for method in [
        "Target.setAutoAttach",
        "Target.getTargetInfo",
        "Target.setDiscoverTargets",
        "Browser.getVersion",
    ] {
        call(&relay, json!({ "id": 10, "method": method }));
    }
}

/// The Lighthouse panel (RB-080-21): `ChildTargetManager.getParentTargetId` asks for the
/// page's own info, `LighthouseProtocolService.attach` attaches a parallel session to that
/// target, and Lighthouse's Puppeteer discovers and auto-attaches on the parallel session.
#[test]
fn lighthouse_gets_a_parallel_session_on_the_frontends_own_target_and_sees_only_it() {
    let mut relay = relay();
    let own = call(
        &relay,
        json!({ "id": 1, "method": "Target.getTargetInfo", "params": { "targetId": "T1" } }),
    );
    assert_eq!(own.session, "PAGE");
    let attach = call(
        &relay,
        json!({ "id": 2, "method": "Target.attachToTarget", "params": { "targetId": "T1", "flatten": true } }),
    );
    assert_eq!((attach.session.as_str(), attach.child), ("PAGE", None));
    let parallel = relay.read_event(&event(
        "Target.attachedToTarget",
        json!({ "sessionId": "S2", "targetInfo": target_info("T1", "page"), "waitingForDebugger": false }),
        Some("PAGE"),
    ));
    assert!(matches!(parallel, ToFrontend::Forward(_)));
    for (method, params) in [
        ("Target.setDiscoverTargets", json!({ "discover": true })),
        (
            "Target.setAutoAttach",
            json!({ "autoAttach": true, "waitForDebuggerOnStart": true, "flatten": true }),
        ),
        (
            "Target.attachToTarget",
            json!({ "targetId": "T1", "flatten": true }),
        ),
        ("Target.getTargetInfo", json!({})),
    ] {
        let on_parallel = call(
            &relay,
            json!({ "id": 3, "method": method, "params": params, "sessionId": "S2" }),
        );
        assert_eq!(on_parallel.session, "S2", "{method}");
    }
    // Discovery tells the frontend of its own target; the other views stay out of sight.
    let created = relay.read_event(&event(
        "Target.targetCreated",
        json!({ "targetInfo": target_info("T1", "page") }),
        Some("S2"),
    ));
    assert_eq!(
        created,
        ToFrontend::Forward(json!({
            "method": "Target.targetCreated",
            "params": { "targetInfo": target_info("T1", "page") },
            "sessionId": "S2",
        }))
    );
    for (method, params) in [
        (
            "Target.targetCreated",
            json!({ "targetInfo": target_info("T2", "page") }),
        ),
        (
            "Target.targetInfoChanged",
            json!({ "targetInfo": target_info("T2", "page") }),
        ),
        ("Target.targetDestroyed", json!({ "targetId": "T2" })),
    ] {
        assert_eq!(
            relay.read_event(&event(method, params, Some("S2"))),
            ToFrontend::Skip
        );
    }
    // Lighthouse done: the parallel session is detached from where it was made.
    let detach = call(
        &relay,
        json!({ "id": 4, "method": "Target.detachFromTarget", "params": { "sessionId": "S2" } }),
    );
    assert_eq!(detach.session, "PAGE");
    relay.read_event(&event(
        "Target.detachedFromTarget",
        json!({ "sessionId": "S2", "targetId": "T1" }),
        Some("PAGE"),
    ));
    let gone = answer(
        &relay,
        json!({ "id": 5, "method": "Runtime.enable", "sessionId": "S2" }),
    );
    assert_eq!(gone["error"]["code"], -32001);
}

#[test]
fn a_frame_the_page_announced_may_be_attached_by_its_target_until_it_detaches() {
    let mut relay = relay();
    let frame = json!({ "targetId": "F1", "flatten": true });
    let refused = answer(
        &relay,
        json!({ "id": 1, "method": "Target.attachToTarget", "params": frame }),
    );
    assert_eq!(refused["error"]["message"], "Not allowed");
    relay.read_event(&event(
        "Target.attachedToTarget",
        json!({ "sessionId": "CHILD", "targetInfo": target_info("F1", "iframe") }),
        Some("PAGE"),
    ));
    call(
        &relay,
        json!({ "id": 2, "method": "Target.attachToTarget", "params": frame }),
    );
    call(
        &relay,
        json!({ "id": 3, "method": "Target.getTargetInfo", "params": { "targetId": "F1" } }),
    );
    let changed = relay.read_event(&event(
        "Target.targetInfoChanged",
        json!({ "targetInfo": target_info("F1", "iframe") }),
        Some("PAGE"),
    ));
    assert!(matches!(changed, ToFrontend::Forward(_)));
    relay.read_event(&event(
        "Target.detachedFromTarget",
        json!({ "sessionId": "CHILD", "targetId": "F1" }),
        Some("PAGE"),
    ));
    let refused = answer(
        &relay,
        json!({ "id": 4, "method": "Target.attachToTarget", "params": frame }),
    );
    assert_eq!(refused["error"]["message"], "Not allowed");
}

/// Inspect mode (RB-080-21) runs in the view itself: the frontend's `Overlay.setInspectMode`
/// reaches the page, and the pick comes back as `Overlay.inspectNodeRequested`.
#[test]
fn inspect_mode_goes_to_the_view_and_the_picked_node_comes_back() {
    let mut relay = relay();
    let mode = call(
        &relay,
        json!({ "id": 1, "method": "Overlay.setInspectMode", "params": { "mode": "searchForNode", "highlightConfig": { "showInfo": true } } }),
    );
    assert_eq!(mode.session, "PAGE");
    let picked = relay.read_event(&event(
        "Overlay.inspectNodeRequested",
        json!({ "backendNodeId": 12 }),
        Some("PAGE"),
    ));
    assert_eq!(
        picked,
        ToFrontend::Forward(
            json!({ "method": "Overlay.inspectNodeRequested", "params": { "backendNodeId": 12 } })
        )
    );
}

#[test]
fn malformed_messages_are_answered_when_they_have_an_id_and_dropped_otherwise() {
    let relay = relay();
    assert_eq!(relay.read_message("not json"), FromFrontend::Drop);
    assert_eq!(relay.read_message("[1, 2]"), FromFrontend::Drop);
    assert_eq!(
        relay.read_message(r#"{ "method": "Page.enable" }"#),
        FromFrontend::Drop
    );
    assert_eq!(
        relay.read_message(r#"{ "id": "x", "method": "Page.enable" }"#),
        FromFrontend::Drop
    );
    let no_method = answer(&relay, json!({ "id": 4 }));
    assert_eq!(no_method["error"]["code"], -32600);
    let bad_params = answer(
        &relay,
        json!({ "id": 5, "method": "Page.enable", "params": [1] }),
    );
    assert_eq!(bad_params["error"]["code"], -32600);
}

#[test]
fn events_of_other_sessions_and_hidden_targets_are_skipped() {
    let mut relay = relay();
    assert_eq!(
        relay.read_event(&event(
            "Page.loadEventFired",
            json!({}),
            Some("HOST-SESSION")
        )),
        ToFrontend::Skip
    );
    assert_eq!(
        relay.read_event(&event("Target.targetCreated", json!({}), None)),
        ToFrontend::Skip
    );
    let hidden = relay.read_event(&event(
        "Target.targetInfoChanged",
        json!({ "targetInfo": { "targetId": "D", "type": "page", "url": "devtools://devtools/bundled/inspector.html" } }),
        Some("PAGE"),
    ));
    assert_eq!(hidden, ToFrontend::Skip);
    let page = relay.read_event(&event(
        "Page.frameNavigated",
        json!({ "frame": {} }),
        Some("PAGE"),
    ));
    assert_eq!(
        page,
        ToFrontend::Forward(json!({ "method": "Page.frameNavigated", "params": { "frame": {} } }))
    );
}

#[test]
fn the_pages_own_detach_ends_the_connection() {
    let mut relay = relay();
    assert_eq!(relay.page_session(), "PAGE");
    assert_eq!(
        relay.read_event(&event(
            "Target.detachedFromTarget",
            json!({ "sessionId": "OTHER" }),
            None
        )),
        ToFrontend::Skip
    );
    assert_eq!(
        relay.read_event(&event(
            "Target.detachedFromTarget",
            json!({ "sessionId": "PAGE" }),
            None
        )),
        ToFrontend::PageGone
    );
}

#[test]
fn the_overlay_counts_as_enabled_once_the_frontend_enabled_it_on_the_page() {
    let relay = relay();
    assert!(!relay.overlay_enabled());
    call(&relay, json!({ "id": 1, "method": "DOM.enable" }));
    assert!(!relay.overlay_enabled(), "another domain");
    let mut relay = relay;
    relay.read_event(&event(
        "Target.attachedToTarget",
        json!({ "sessionId": "CHILD", "targetInfo": target_info("T2", "iframe") }),
        Some("PAGE"),
    ));
    call(
        &relay,
        json!({ "id": 2, "method": "Overlay.enable", "sessionId": "CHILD" }),
    );
    assert!(
        !relay.overlay_enabled(),
        "a child's overlay is not the page's"
    );
    let enable = call(&relay, json!({ "id": 3, "method": "Overlay.enable" }));
    assert!(
        !relay.overlay_enabled(),
        "not before the browser answered the enable"
    );
    relay.on_answered(&enable, false);
    assert!(!relay.overlay_enabled(), "a failed enable");
    relay.on_answered(&enable, true);
    assert!(relay.overlay_enabled());
    let other = call(&relay, json!({ "id": 4, "method": "DOM.getDocument" }));
    relay.on_answered(&other, false);
    assert!(
        relay.overlay_enabled(),
        "another call's answer changes nothing"
    );
    call(&relay, json!({ "id": 5, "method": "Overlay.disable" }));
    assert!(!relay.overlay_enabled(), "disabled again");
}

#[test]
fn the_inspect_event_is_what_chromium_sends_the_frontend() {
    assert_eq!(
        Relay::inspect_event(42),
        json!({ "method": "Overlay.inspectNodeRequested", "params": { "backendNodeId": 42 } })
    );
}
