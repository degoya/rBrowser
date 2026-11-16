use serde_json::json;

use super::*;
use crate::cdp::CdpEvent;
use crate::devtools::relay::{Relay, ToFrontend};

fn created(id: i64, unique: &str, name: &str, kind: &str) -> Value {
    json!({ "context": {
        "id": id,
        "origin": "https://a.test",
        "name": name,
        "uniqueId": unique,
        "auxData": { "isDefault": kind == "default", "type": kind, "frameId": "F1" },
    } })
}

#[test]
fn every_world_of_rbrowser_is_its_own() {
    for name in [
        rb_sync::event::WORLD_NAME,
        crate::devtools::inspect::INSPECT_WORLD,
        crate::reload::script::RELOAD_WORLD,
        crate::view_footer::read::READ_WORLD,
        rb_visual::prepare::WORLD,
        &rb_plugin_host::world_name("rulers-guides"),
        &rb_plugin_host::world_name("snippets"),
    ] {
        assert!(is_own_world(name), "{name}");
    }
}

#[test]
fn no_world_of_the_page_or_an_extension_is_its_own() {
    for name in [
        "",
        "Rbrowser",
        "rbrowserx",
        "my-rbrowser-tools",
        "React Developer Tools",
        "rbplugin",
    ] {
        assert!(!is_own_world(name), "{name}");
    }
}

#[test]
fn hides_an_isolated_world_of_rbrowser_and_its_end() {
    let mut worlds = OwnWorlds::default();
    let method = "Runtime.executionContextCreated";
    assert!(worlds.hides("PAGE", method, &created(3, "u3", "rbrowser", "isolated")));
    assert!(worlds.hides(
        "PAGE",
        method,
        &created(4, "u4", "rbplugin-peek", "isolated")
    ));
    let destroyed = "Runtime.executionContextDestroyed";
    assert!(worlds.hides(
        "PAGE",
        destroyed,
        &json!({ "executionContextId": 3, "executionContextUniqueId": "u3" })
    ));
    // By the unique id alone, as a newer Chromium may send it.
    assert!(worlds.hides(
        "PAGE",
        destroyed,
        &json!({ "executionContextUniqueId": "u4" })
    ));
    // Each end once: the same id later is a context the frontend knows.
    assert!(!worlds.hides("PAGE", destroyed, &json!({ "executionContextId": 3 })));
}

#[test]
fn keeps_the_pages_contexts_and_extensions() {
    let mut worlds = OwnWorlds::default();
    let method = "Runtime.executionContextCreated";
    // The page's own context, even one a page named like rBrowser's world.
    assert!(!worlds.hides("PAGE", method, &created(1, "u1", "", "default")));
    assert!(!worlds.hides("PAGE", method, &created(2, "u2", "rbrowser", "default")));
    // An extension's content script world.
    assert!(!worlds.hides(
        "PAGE",
        method,
        &created(5, "u5", "React Developer Tools", "isolated")
    ));
    // A worker's context has no auxData type.
    assert!(!worlds.hides(
        "PAGE",
        method,
        &json!({ "context": { "id": 6, "name": "rbrowser", "uniqueId": "u6" } })
    ));
    let destroyed = "Runtime.executionContextDestroyed";
    for id in [1, 2, 5, 6] {
        assert!(!worlds.hides("PAGE", destroyed, &json!({ "executionContextId": id })));
    }
    assert!(!worlds.hides("PAGE", "Runtime.consoleAPICalled", &json!({})));
}

#[test]
fn context_ids_are_per_session() {
    let mut worlds = OwnWorlds::default();
    assert!(worlds.hides(
        "CHILD",
        "Runtime.executionContextCreated",
        &created(3, "c3", "rbrowser", "isolated")
    ));
    // Id 3 of the page is another context.
    assert!(!worlds.hides(
        "PAGE",
        "Runtime.executionContextDestroyed",
        &json!({ "executionContextId": 3 })
    ));
    assert!(!worlds.hides("CHILD", "Runtime.executionContextsCleared", &json!({})));
    // Cleared: the session's contexts are gone, their ids free again.
    assert!(!worlds.hides(
        "CHILD",
        "Runtime.executionContextDestroyed",
        &json!({ "executionContextId": 3 })
    ));
    worlds.hides(
        "CHILD",
        "Runtime.executionContextCreated",
        &created(7, "c7", "rbrowser-inspect", "isolated"),
    );
    worlds.forget("CHILD");
    assert!(!worlds.hides(
        "CHILD",
        "Runtime.executionContextDestroyed",
        &json!({ "executionContextId": 7 })
    ));
}

fn event(method: &str, params: Value, session: Option<&str>) -> CdpEvent {
    CdpEvent {
        method: method.to_owned(),
        params,
        session_id: session.map(str::to_owned),
    }
}

#[test]
fn the_relay_shows_the_console_only_the_pages_contexts() {
    let mut relay = Relay::new("PAGE".to_owned(), "T1".to_owned());
    let page = event(
        "Runtime.executionContextCreated",
        created(1, "u1", "", "default"),
        Some("PAGE"),
    );
    assert!(matches!(relay.read_event(&page), ToFrontend::Forward(_)));
    for name in ["rbrowser", "rbrowser-inspect", "rbplugin-color-picker"] {
        let own = event(
            "Runtime.executionContextCreated",
            created(2, "u2", name, "isolated"),
            Some("PAGE"),
        );
        assert_eq!(relay.read_event(&own), ToFrontend::Skip, "{name}");
        let gone = event(
            "Runtime.executionContextDestroyed",
            json!({ "executionContextId": 2, "executionContextUniqueId": "u2" }),
            Some("PAGE"),
        );
        assert_eq!(relay.read_event(&gone), ToFrontend::Skip, "{name}");
    }
    let page_gone = event(
        "Runtime.executionContextDestroyed",
        json!({ "executionContextId": 1, "executionContextUniqueId": "u1" }),
        Some("PAGE"),
    );
    assert!(matches!(
        relay.read_event(&page_gone),
        ToFrontend::Forward(_)
    ));
}

#[test]
fn the_relay_hides_rbrowsers_worlds_in_an_announced_frame_too() {
    let mut relay = Relay::new("PAGE".to_owned(), "T1".to_owned());
    let attached = event(
        "Target.attachedToTarget",
        json!({
            "sessionId": "CHILD",
            "targetInfo": { "targetId": "F2", "type": "iframe", "url": "https://b.test/" },
        }),
        Some("PAGE"),
    );
    assert!(matches!(
        relay.read_event(&attached),
        ToFrontend::Forward(_)
    ));
    let own = event(
        "Runtime.executionContextCreated",
        created(4, "c4", "rbplugin-peek", "isolated"),
        Some("CHILD"),
    );
    assert_eq!(relay.read_event(&own), ToFrontend::Skip);
    let frame = event(
        "Runtime.executionContextCreated",
        created(5, "c5", "", "default"),
        Some("CHILD"),
    );
    assert!(matches!(relay.read_event(&frame), ToFrontend::Forward(_)));
}
