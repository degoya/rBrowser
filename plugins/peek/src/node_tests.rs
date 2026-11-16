use serde_json::json;

use super::*;
use crate::testing::FakeHost;

fn chromium() -> FakeHost {
    let mut host = FakeHost::default();
    host.answer(
        "Accessibility.getFullAXTree",
        json!({ "nodes": [{ "nodeId": "1", "role": { "value": "RootWebArea" }, "backendDOMNodeId": 5 }] }),
    )
    .answer("DOM.getDocument", json!({ "root": { "nodeId": 1 } }))
    .answer("DOM.querySelector", json!({ "nodeId": 42 }))
    .answer("DOM.getNodeForLocation", json!({ "backendNodeId": 7, "frameId": "f" }))
    .answer("DOM.pushNodesByBackendIdsToFrontend", json!({ "nodeIds": [43] }))
    .answer(
        "CSS.getPlatformFontsForNode",
        json!({ "fonts": [
            { "familyName": "Arial", "postScriptName": "ArialMT", "isCustomFont": false, "glyphCount": 3 },
            { "familyName": "Segoe UI", "postScriptName": "SegoeUI", "isCustomFont": false, "glyphCount": 12 },
        ] }),
    )
    .answer(
        "Accessibility.getPartialAXTree",
        json!({ "nodes": [
            { "nodeId": "1", "ignored": false, "role": { "type": "role", "value": "button" },
              "name": { "type": "computedString", "value": "Send" } },
        ] }),
    );
    host
}

/// Review finding L6 of Charge 6: the session is shared, so peek switches no domain on and never
/// requests the document anew while it is bound (that would drop every node id handed out).
fn assert_quiet(host: &FakeHost) {
    for method in host.methods() {
        assert!(!method.ends_with(".enable"), "{method}");
        assert!(!method.ends_with(".disable"), "{method}");
    }
}

#[test]
fn a_selector_is_found_from_the_bound_document_without_requesting_it_again() {
    let mut host = chromium();
    let extras = Extras::read(&mut host, 2, &Locator::Selector("#send".to_owned()));
    assert_eq!(
        host.methods(),
        [
            "Accessibility.getFullAXTree",
            "DOM.pushNodesByBackendIdsToFrontend",
            "DOM.querySelector",
            "CSS.getPlatformFontsForNode",
            "Accessibility.getPartialAXTree",
        ]
    );
    assert_eq!(host.cdp_sent[0].2, json!({ "depth": 1 }));
    assert_eq!(host.cdp_sent[1].2, json!({ "backendNodeIds": [5] }));
    assert_eq!(
        host.cdp_sent[2].2,
        json!({ "nodeId": 43, "selector": "#send" })
    );
    assert_eq!(host.cdp_sent[3].2, json!({ "nodeId": 42 }));
    assert!(host.cdp_sent.iter().all(|(view, _, _)| *view == 2));
    assert_quiet(&host);
    // The font with the most glyphs first.
    let fonts = extras.rendered_fonts.expect("fonts");
    assert_eq!(fonts[0].family, "Segoe UI");
    assert_eq!(fonts[1].post_script_name, "ArialMT");
    assert_eq!(extras.role.as_deref(), Some("button"));
    assert_eq!(extras.name.as_deref(), Some("Send"));
}

#[test]
fn the_document_is_requested_only_when_nobody_has_it() {
    // Unbound, nothing holds a node id that the request could drop.
    let mut host = chromium();
    host.fail_once("DOM.pushNodesByBackendIdsToFrontend");
    let extras = Extras::read(&mut host, 1, &Locator::Selector("#send".to_owned()));
    assert_eq!(
        &host.methods()[..4],
        [
            "Accessibility.getFullAXTree",
            "DOM.pushNodesByBackendIdsToFrontend",
            "DOM.getDocument",
            "DOM.querySelector",
        ]
    );
    assert_eq!(host.cdp_sent[2].2, json!({ "depth": 0 }));
    assert_eq!(
        host.cdp_sent[3].2,
        json!({ "nodeId": 1, "selector": "#send" })
    );
    assert!(extras.rendered_fonts.is_some());
    assert_quiet(&host);
}

#[test]
fn a_point_is_found_through_the_backend_node() {
    let mut host = chromium();
    let extras = Extras::read(&mut host, 1, &Locator::Point { x: 10.6, y: 20.2 });
    assert_eq!(
        &host.methods()[..3],
        [
            "DOM.getNodeForLocation",
            "DOM.pushNodesByBackendIdsToFrontend",
            "CSS.getPlatformFontsForNode",
        ]
    );
    assert_eq!(
        host.cdp_sent[0].2,
        json!({ "x": 11, "y": 20, "includeUserAgentShadowDOM": false })
    );
    assert_eq!(host.cdp_sent[1].2, json!({ "backendNodeIds": [7] }));
    assert_eq!(host.cdp_sent[2].2, json!({ "nodeId": 43 }));
    assert!(extras.rendered_fonts.is_some());
    assert_quiet(&host);
}

#[test]
fn a_point_in_an_unbound_document_requests_it_once_and_pushes_again() {
    let mut host = chromium();
    host.fail_once("DOM.pushNodesByBackendIdsToFrontend");
    let extras = Extras::read(&mut host, 1, &Locator::Point { x: 1.0, y: 2.0 });
    assert_eq!(
        &host.methods()[..4],
        [
            "DOM.getNodeForLocation",
            "DOM.pushNodesByBackendIdsToFrontend",
            "DOM.getDocument",
            "DOM.pushNodesByBackendIdsToFrontend",
        ]
    );
    assert!(extras.rendered_fonts.is_some());
    assert_quiet(&host);
}

#[test]
fn fonts_the_css_domain_will_not_tell_stay_unknown_and_nothing_is_switched_on() {
    let mut host = chromium();
    host.fail("CSS.getPlatformFontsForNode");
    let extras = Extras::read(&mut host, 1, &Locator::Selector("p".to_owned()));
    assert_eq!(extras.rendered_fonts, None);
    assert_eq!(
        host.methods()
            .iter()
            .filter(|method| **method == "CSS.getPlatformFontsForNode")
            .count(),
        1
    );
    assert_quiet(&host);
    // The role still comes.
    assert_eq!(extras.role.as_deref(), Some("button"));
}

#[test]
fn an_element_not_found_gives_nothing_and_no_error() {
    let mut host = chromium();
    host.answer("DOM.querySelector", json!({ "nodeId": 0 }));
    let extras = Extras::read(&mut host, 1, &Locator::Selector("#gone".to_owned()));
    assert_eq!(extras, Extras::default());
    assert_eq!(
        host.methods(),
        [
            "Accessibility.getFullAXTree",
            "DOM.pushNodesByBackendIdsToFrontend",
            "DOM.querySelector"
        ]
    );
}

#[test]
fn without_cdp_firefox_and_webkit_get_nothing() {
    let mut host = FakeHost::default();
    let extras = Extras::read(&mut host, 1, &Locator::Selector("p".to_owned()));
    assert_eq!(extras, Extras::default());
    assert_eq!(host.methods(), ["Accessibility.getFullAXTree"]);
}

#[test]
fn an_ignored_ax_node_has_no_role_and_long_texts_are_cut() {
    let mut host = chromium();
    host.answer(
        "Accessibility.getPartialAXTree",
        json!({ "nodes": [{ "nodeId": "1", "ignored": true, "role": { "value": "none" } }] }),
    );
    let extras = Extras::read(&mut host, 1, &Locator::Selector("div".to_owned()));
    assert_eq!(extras.role, None);
    let long = "n".repeat(1000);
    host.answer(
        "Accessibility.getPartialAXTree",
        json!({ "nodes": [{ "role": { "value": "link" }, "name": { "value": long } }] }),
    );
    let extras = Extras::read(&mut host, 1, &Locator::Selector("a".to_owned()));
    assert_eq!(extras.name.map(|name| name.chars().count()), Some(MAX_TEXT));
}

#[test]
fn fonts_are_capped_and_their_json_named_for_the_tool() {
    let many: Vec<_> = (0..40)
        .map(|index| json!({ "familyName": format!("F{index}"), "glyphCount": index }))
        .collect();
    let mut host = chromium();
    host.answer("CSS.getPlatformFontsForNode", json!({ "fonts": many }));
    let extras = Extras::read(&mut host, 1, &Locator::Selector("p".to_owned()));
    let fonts = extras.rendered_fonts.expect("fonts");
    assert_eq!(fonts.len(), MAX_FONTS);
    assert_eq!(
        fonts[0].to_json(),
        json!({ "family": "F39", "postScriptName": "", "custom": false, "glyphs": 39 })
    );
}
