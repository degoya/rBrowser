use serde_json::json;

use super::{is_lifecycle, isolated};
use crate::cdp::CdpEvent;

fn targets() -> serde_json::Value {
    json!({ "targetInfos": [
        { "targetId": "view-1", "type": "page", "browserContextId": "global" },
        { "targetId": "shell", "type": "page", "browserContextId": "global" },
        { "targetId": "view-2", "type": "page", "browserContextId": "session-login" },
        { "targetId": "probe", "type": "page", "browserContextId": "fresh-1" },
    ] })
}

#[test]
fn a_context_of_its_own_is_the_probes() {
    assert_eq!(isolated(&targets(), "probe").as_deref(), Some("fresh-1"));
}

#[test]
fn a_context_a_view_shares_is_never_the_probes() {
    // The global context (a CEF context that fell back to it) and a named session's.
    assert_eq!(isolated(&targets(), "view-1"), None);
    let mut shared = targets();
    shared["targetInfos"][3]["browserContextId"] = json!("session-login");
    assert_eq!(isolated(&shared, "probe"), None);
    shared["targetInfos"][3]["browserContextId"] = json!("global");
    assert_eq!(isolated(&shared, "probe"), None);
}

#[test]
fn a_target_without_a_context_or_unlisted_is_refused() {
    let mut none = targets();
    none["targetInfos"][3]
        .as_object_mut()
        .expect("info")
        .remove("browserContextId");
    assert_eq!(isolated(&none, "probe"), None);
    none["targetInfos"][3]["browserContextId"] = json!("");
    assert_eq!(isolated(&none, "probe"), None);
    assert_eq!(isolated(&targets(), "gone"), None);
    assert_eq!(isolated(&json!({}), "probe"), None);
}

#[test]
fn only_the_probes_own_document_counts_as_loaded() {
    let event = |session: &str, name: &str, loader: &str| CdpEvent {
        method: "Page.lifecycleEvent".to_owned(),
        params: json!({ "name": name, "frameId": "F", "loaderId": loader }),
        session_id: Some(session.to_owned()),
    };
    let ids = ("F", "L");
    assert!(is_lifecycle(&event("S", "load", "L"), "S", ids, "load"));
    // A view's page, an earlier document, another event.
    assert!(!is_lifecycle(&event("V", "load", "L"), "S", ids, "load"));
    assert!(!is_lifecycle(&event("S", "load", "L0"), "S", ids, "load"));
    assert!(!is_lifecycle(
        &event("S", "DOMContentLoaded", "L"),
        "S",
        ids,
        "load"
    ));
}
