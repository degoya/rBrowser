use std::collections::BTreeSet;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use rb_engine::ViewId;
use serde_json::{Value, json};

use super::*;
use crate::force_state::cdp::{CHAIN, FIND, TARGET_OF};
use crate::force_state::testing::{Replying, page};

fn hover() -> BTreeSet<PseudoClass> {
    [PseudoClass::Hover].into_iter().collect()
}

fn view(id: u32) -> ViewPage {
    ViewPage {
        view: ViewId(id),
        page: Some(format!("PAGE{id}")),
        agent: Some((format!("PAGE{id}"), i64::from(id) * 10)),
    }
}

fn source() -> Source {
    Source {
        view: ViewId(1),
        backend: 42,
        agent: Some(("PAGE1".to_owned(), 10)),
    }
}

/// Two views: the sync agent names the element in view 1 and finds it (backend 77) in view 2;
/// each session of rBrowser's opened is `OWN<n>`, in order.
fn two_views() -> impl Fn(&str, &Value) -> Result<Value, String> + Send + Sync + 'static {
    let opened = AtomicU64::new(0);
    move |method: &str, params: &Value| match method {
        "Target.attachToTarget" => {
            let n = opened.fetch_add(1, Ordering::Relaxed) + 1;
            Ok(json!({ "sessionId": format!("OWN{n}") }))
        }
        "Runtime.callFunctionOn" if params["functionDeclaration"] == TARGET_OF => Ok(json!({
            "result": { "value": { "element": { "kind": "css", "path": "li" }, "offset": {} } }
        })),
        "Runtime.callFunctionOn" if params["functionDeclaration"] == FIND => {
            Ok(json!({ "result": { "objectId": "B-el" } }))
        }
        "DOM.describeNode" => Ok(json!({ "node": { "backendNodeId": 77 } })),
        _ => page(method, params),
    }
}

/// `(session, params)` of every `CSS.forcePseudoState`, in order.
fn forced(end: &Replying) -> Vec<(String, Value)> {
    end.sent()
        .into_iter()
        .filter(|message| message["method"] == "CSS.forcePseudoState")
        .map(|message| {
            let session = message["sessionId"].as_str().unwrap_or_default().to_owned();
            (session, message["params"].clone())
        })
        .collect()
}

/// The menu's ":hover" in all views: the element under the pointer in view 1, the same one in
/// view 2 through the sync agent; each with its ancestors, each on its own session.
#[tokio::test]
async fn hover_reaches_the_element_and_its_ancestors_in_every_view() {
    let (end, router) = Replying::connect(two_views());
    let registry = Mutex::new(Registry::default());
    let outcomes = force(
        &router,
        &registry,
        &[view(1), view(2)],
        &Element::From(source()),
        &Request::Toggle(PseudoClass::Hover),
    )
    .await;
    assert_eq!(
        outcomes,
        [
            (ViewId(1), Outcome::Forced(hover())),
            (ViewId(2), Outcome::Forced(hover())),
        ]
    );
    let forced = forced(&end);
    assert_eq!(
        forced.len(),
        6,
        "three nodes in each of two views: {forced:?}"
    );
    assert!(
        forced
            .iter()
            .all(|(_, params)| params["forcedPseudoClasses"] == json!(["hover"]))
    );
    for session in ["OWN1", "OWN2"] {
        assert_eq!(
            forced.iter().filter(|(on, _)| on == session).count(),
            3,
            "each view on its own session: {forced:?}"
        );
    }
    // The element in view 2 is the one the agent found.
    assert_eq!(
        end.params("DOM.resolveNode")
            .last()
            .map(|params| params["backendNodeId"].clone()),
        Some(json!(77))
    );
    // The same toggle again takes it back, in both views.
    let outcomes = force(
        &router,
        &registry,
        &[view(1), view(2)],
        &Element::From(source()),
        &Request::Toggle(PseudoClass::Hover),
    )
    .await;
    assert_eq!(
        outcomes,
        [
            (ViewId(1), Outcome::Forced(BTreeSet::new())),
            (ViewId(2), Outcome::Forced(BTreeSet::new())),
        ]
    );
    assert!(lock(&registry).forced_views().is_empty());
    assert_eq!(
        end.calls()
            .iter()
            .filter(|(_, method)| method == "Target.attachToTarget")
            .count(),
        2,
        "a view's session is opened once and kept"
    );
}

#[tokio::test]
async fn a_view_without_the_element_says_so_and_the_others_are_forced() {
    let base = two_views();
    let (_, router) = Replying::connect(move |method, params| match method {
        "Runtime.callFunctionOn" if params["functionDeclaration"] == FIND => {
            Ok(json!({ "result": { "subtype": "null", "value": null } }))
        }
        _ => base(method, params),
    });
    let registry = Mutex::new(Registry::default());
    let outcomes = force(
        &router,
        &registry,
        &[view(1), view(2)],
        &Element::From(source()),
        &Request::Set(hover()),
    )
    .await;
    assert_eq!(
        outcomes,
        [
            (ViewId(1), Outcome::Forced(hover())),
            (ViewId(2), Outcome::Missing),
        ]
    );
}

#[tokio::test]
async fn a_view_without_cdp_or_without_an_agent_fails_alone() {
    let (_, router) = Replying::connect(two_views());
    let registry = Mutex::new(Registry::default());
    let firefox = ViewPage {
        view: ViewId(3),
        page: None,
        agent: None,
    };
    let no_agent = ViewPage {
        agent: None,
        ..view(2)
    };
    let outcomes = force(
        &router,
        &registry,
        &[view(1), firefox, no_agent],
        &Element::From(source()),
        &Request::Set(hover()),
    )
    .await;
    assert_eq!(outcomes[0], (ViewId(1), Outcome::Forced(hover())));
    assert!(
        matches!(&outcomes[1], (ViewId(3), Outcome::Failed(why)) if why.contains("Chromium")),
        "{outcomes:?}"
    );
    assert!(
        matches!(&outcomes[2], (ViewId(2), Outcome::Failed(why)) if why.contains("sync agent")),
        "{outcomes:?}"
    );
}

#[tokio::test]
async fn a_selector_is_looked_up_in_each_view_on_rbrowsers_session() {
    let (end, router) = Replying::connect(|method, params| match method {
        "DOM.querySelector" => Ok(json!({ "nodeId": 5 })),
        "DOM.describeNode" => Ok(json!({ "node": { "backendNodeId": 55 } })),
        _ => page(method, params),
    });
    let registry = Mutex::new(Registry::default());
    let outcomes = force(
        &router,
        &registry,
        &[view(1)],
        &Element::Selector("nav li".to_owned()),
        &Request::Set(hover()),
    )
    .await;
    assert_eq!(outcomes, [(ViewId(1), Outcome::Forced(hover()))]);
    assert_eq!(
        end.params("DOM.querySelector")[0],
        json!({ "nodeId": 1, "selector": "nav li" })
    );
    assert_eq!(end.params("DOM.resolveNode")[0]["backendNodeId"], 55);
    assert!(
        end.params("Runtime.callFunctionOn")
            .iter()
            .all(|call| call["functionDeclaration"] == CHAIN),
        "no sync agent for a selector"
    );
}

/// After a navigation the document is asked for again on the same session.
#[tokio::test]
async fn after_a_new_document_the_session_asks_for_it_again() {
    let (end, router) = Replying::connect(page);
    let registry = Mutex::new(Registry::default());
    let only = Element::From(Source {
        agent: None,
        ..source()
    });
    force(
        &router,
        &registry,
        &[view(1)],
        &only,
        &Request::Set(hover()),
    )
    .await;
    lock(&registry).document_updated("OWN");
    force(
        &router,
        &registry,
        &[view(1)],
        &only,
        &Request::Set(hover()),
    )
    .await;
    let methods: Vec<String> = end.calls().into_iter().map(|(_, method)| method).collect();
    assert_eq!(
        methods
            .iter()
            .filter(|method| *method == "DOM.getDocument")
            .count(),
        2
    );
    assert_eq!(
        methods
            .iter()
            .filter(|method| *method == "Target.attachToTarget")
            .count(),
        1
    );
}

/// The reset takes every node back and ends the session, even when a node is gone.
#[tokio::test]
async fn the_reset_empties_every_node_and_detaches_even_after_a_failure() {
    let (end, router) = Replying::connect(page);
    let registry = Mutex::new(Registry::default());
    let only = Element::From(Source {
        agent: None,
        ..source()
    });
    force(
        &router,
        &registry,
        &[view(1)],
        &only,
        &Request::Set(hover()),
    )
    .await;
    let (_, router2) = Replying::connect(|method, _| {
        if method == "CSS.forcePseudoState" {
            Err("Could not find node with given id".to_owned())
        } else {
            Ok(json!({}))
        }
    });
    let reset_views = reset(&router, &registry, &[ViewId(1), ViewId(2)]).await;
    assert_eq!(reset_views, [ViewId(1)]);
    let emptied: Vec<Value> = end
        .params("CSS.forcePseudoState")
        .into_iter()
        .filter(|params| params["forcedPseudoClasses"] == json!([]))
        .collect();
    assert_eq!(emptied.len(), 3, "the element and its two ancestors");
    assert_eq!(
        end.params("Target.detachFromTarget"),
        [json!({ "sessionId": "OWN" })]
    );
    assert_eq!(lock(&registry).session(ViewId(1)), None);
    // A failing reset still detaches.
    let registry = Mutex::new(Registry::default());
    lock(&registry).open(ViewId(1), "S9".to_owned(), 1);
    lock(&registry).apply(ViewId(1), 42, vec![10], hover());
    reset(&router2, &registry, &[ViewId(1)]).await;
    assert_eq!(lock(&registry).session(ViewId(1)), None);
}
