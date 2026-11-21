use rb_plugin_api::{Failure, FailureKind, codes};
use serde_json::json;

use super::*;
use crate::fake::FakeHost;

#[test]
fn targets_are_checked() {
    let ok = targets(&json!([{ "view": 1, "selector": "#a" }, { "view": 2, "backendNodeId": 7 }]))
        .expect("targets");
    assert_eq!(ok[0], (1, Locator::Selector("#a".to_owned())));
    assert_eq!(ok[1], (2, Locator::Backend(7)));
    for bad in [
        json!([]),
        json!("x"),
        json!([{ "view": -1, "selector": "a" }]),
        json!([{ "view": 1, "selector": "  " }]),
        json!([{ "view": 1, "selector": "a\nb" }]),
        json!([{ "view": 1, "selector": "a".repeat(MAX_SELECTOR_CHARS + 1) }]),
        json!([{ "view": 1 }]),
    ] {
        let failure = targets(&bad).expect_err("refused");
        assert_eq!(failure.code(), Some(codes::INVALID), "{bad}");
    }
}

#[test]
fn a_selector_is_a_cdp_parameter_and_the_element_is_scrolled_and_highlighted() {
    let mut host = FakeHost::default();
    // Someone requested the document already: it is found without requesting it again, which
    // would drop every node id the host and the other plugins hold (review L2 of Charge 7).
    host.cdp.insert(
        "Accessibility.getFullAXTree".to_owned(),
        Ok(json!({ "nodes": [{ "backendDOMNodeId": 5 }] })),
    );
    host.cdp.insert(
        "DOM.pushNodesByBackendIdsToFrontend".to_owned(),
        Ok(json!({ "nodeIds": [1] })),
    );
    host.cdp
        .insert("DOM.querySelector".to_owned(), Ok(json!({ "nodeId": 9 })));
    let selector = "a[href=\"');alert(1)//\"]";
    assert!(mark(&mut host, 3, &Locator::Selector(selector.to_owned())).expect("marked"));
    assert_eq!(
        host.methods(),
        [
            "DOM.enable",
            "Overlay.enable",
            "Accessibility.getFullAXTree",
            "DOM.pushNodesByBackendIdsToFrontend",
            "DOM.querySelector",
            "DOM.scrollIntoViewIfNeeded",
            "Overlay.highlightNode"
        ]
    );
    assert_eq!(host.cdp_sent[3].2["backendNodeIds"], json!([5]));
    assert_eq!(host.cdp_sent[4].2["nodeId"], 1);
    assert_eq!(host.cdp_sent[4].2["selector"], selector);
    assert_eq!(host.cdp_sent[6].2["nodeId"], 9);
    assert!(host.evaluated.is_empty(), "no script");
}

#[test]
fn the_document_is_requested_only_when_nobody_has() {
    let mut host = FakeHost::default();
    host.cdp.insert(
        "Accessibility.getFullAXTree".to_owned(),
        Ok(json!({ "nodes": [{ "backendDOMNodeId": 5 }] })),
    );
    host.cdp.insert(
        "DOM.pushNodesByBackendIdsToFrontend".to_owned(),
        Err(Failure::coded(FailureKind::Other, codes::HOST_FAILED)),
    );
    host.cdp.insert(
        "DOM.getDocument".to_owned(),
        Ok(json!({ "root": { "nodeId": 1 } })),
    );
    host.cdp
        .insert("DOM.querySelector".to_owned(), Ok(json!({ "nodeId": 9 })));
    assert!(mark(&mut host, 3, &Locator::Selector("#a".to_owned())).expect("marked"));
    let methods = host.methods();
    assert_eq!(
        methods.iter().filter(|m| **m == "DOM.getDocument").count(),
        1
    );
    assert_eq!(methods[5], "DOM.querySelector");
    assert_eq!(host.cdp_sent[5].2["nodeId"], 1);
}

#[test]
fn a_missing_element_clears_the_old_mark() {
    let mut host = FakeHost::default();
    host.cdp.insert(
        "DOM.getDocument".to_owned(),
        Ok(json!({ "root": { "nodeId": 1 } })),
    );
    host.cdp
        .insert("DOM.querySelector".to_owned(), Ok(json!({ "nodeId": 0 })));
    assert!(!mark(&mut host, 1, &Locator::Selector("#gone".to_owned())).expect("answered"));
    assert_eq!(host.methods().last(), Some(&"Overlay.hideHighlight"));
}

#[test]
fn an_accessibility_node_is_marked_by_its_backend_id() {
    let mut host = FakeHost::default();
    assert!(mark(&mut host, 1, &Locator::Backend(42)).expect("marked"));
    assert_eq!(host.cdp_sent[3].2["backendNodeId"], 42);
    assert!(!host.methods().contains(&"DOM.querySelector"));
}
