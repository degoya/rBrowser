use serde_json::json;

use super::*;
use crate::force_state::testing::{Replying, page};

#[tokio::test]
async fn a_session_of_rbrowsers_own_is_attached_to_the_views_target_with_dom_and_css() {
    let (end, router) = Replying::connect(page);
    let (session, root) = open_session(&router, "PAGE").await.expect("opened");
    assert_eq!((session.as_str(), root), ("OWN", 1));
    let calls: Vec<(String, String)> = end.calls();
    let expected: Vec<(String, String)> = [
        ("PAGE", "Target.getTargetInfo"),
        ("-", "Target.attachToTarget"),
        ("OWN", "DOM.enable"),
        ("OWN", "CSS.enable"),
        ("OWN", "DOM.getDocument"),
    ]
    .iter()
    .map(|(session, method)| ((*session).to_owned(), (*method).to_owned()))
    .collect();
    assert_eq!(
        calls, expected,
        "CSS needs DOM; the document is asked for once"
    );
    assert_eq!(
        end.params("Target.attachToTarget")[0],
        json!({ "targetId": "T1", "flatten": true })
    );
}

#[tokio::test]
async fn a_session_that_does_not_open_is_detached_again() {
    let (end, router) = Replying::connect(|method, params| {
        if method == "CSS.enable" {
            return Err("CSS agent unavailable".to_owned());
        }
        page(method, params)
    });
    let error = open_session(&router, "PAGE").await.expect_err("no CSS");
    assert!(error.to_string().contains("CSS agent"), "{error}");
    assert_eq!(
        end.params("Target.detachFromTarget"),
        [json!({ "sessionId": "OWN" })]
    );
}

/// The element first, then each ancestor, in the order of the array - not of the answer.
#[tokio::test]
async fn the_chain_is_the_element_then_its_ancestors_as_node_ids_of_the_session() {
    let (end, router) = Replying::connect(page);
    let chain = chain_of(&router, "OWN", 42).await.expect("a chain");
    assert_eq!(chain, [10, 9, 8]);
    assert_eq!(
        end.params("DOM.resolveNode")[0],
        json!({ "backendNodeId": 42, "objectGroup": GROUP })
    );
    let call = &end.params("Runtime.callFunctionOn")[0];
    assert_eq!(call["objectId"], "O-el");
    assert_eq!(call["returnByValue"], false);
    assert_eq!(
        end.params("Runtime.getProperties")[0],
        json!({ "objectId": "O-arr", "ownProperties": true })
    );
    assert_eq!(
        end.calls().last().map(|(_, method)| method.as_str()),
        Some("Runtime.releaseObjectGroup"),
        "the objects are let go"
    );
    assert!(end.calls().iter().all(|(session, _)| session == "OWN"));
}

#[tokio::test]
async fn an_element_that_is_gone_is_an_error_and_the_objects_are_released() {
    let (end, router) = Replying::connect(|method, params| {
        if method == "DOM.resolveNode" {
            return Err("No node with given id found".to_owned());
        }
        page(method, params)
    });
    assert!(chain_of(&router, "OWN", 42).await.is_err());
    assert_eq!(
        end.calls().last().map(|(_, method)| method.as_str()),
        Some("Runtime.releaseObjectGroup")
    );
}

#[test]
fn the_chain_walks_up_through_shadow_roots_and_frames() {
    assert!(CHAIN.trim_start().starts_with("function ()"));
    assert!(CHAIN.contains("parentElement"));
    assert!(CHAIN.contains("root.host"));
    assert!(CHAIN.contains("frameElement"));
    assert!(
        CHAIN.contains("chain.length < 256"),
        "a bound on a broken page"
    );
}

#[tokio::test]
async fn each_node_gets_its_whole_set_in_order() {
    let (end, router) = Replying::connect(page);
    let changes = [
        NodeChange {
            node: 8,
            classes: vec!["hover"],
        },
        NodeChange {
            node: 10,
            classes: vec![],
        },
    ];
    force(&router, "OWN", &changes).await.expect("forced");
    assert_eq!(
        end.params("CSS.forcePseudoState"),
        [
            json!({ "nodeId": 8, "forcedPseudoClasses": ["hover"] }),
            json!({ "nodeId": 10, "forcedPseudoClasses": [] }),
        ]
    );
}

#[tokio::test]
async fn a_selector_names_the_backend_node_of_its_first_match_or_none() {
    let (end, router) = Replying::connect(|method, params| match method {
        "DOM.querySelector" if params["selector"] == "nav li" => Ok(json!({ "nodeId": 5 })),
        "DOM.querySelector" => Ok(json!({ "nodeId": 0 })),
        "DOM.describeNode" => Ok(json!({ "node": { "backendNodeId": 77 } })),
        _ => Ok(json!({})),
    });
    let found = selector_node(&router, "OWN", 1, "nav li")
        .await
        .expect("ok");
    assert_eq!(found, Some(77));
    assert_eq!(
        end.params("DOM.querySelector")[0],
        json!({ "nodeId": 1, "selector": "nav li" })
    );
    assert_eq!(end.params("DOM.describeNode")[0], json!({ "nodeId": 5 }));
    let none = selector_node(&router, "OWN", 1, "#missing")
        .await
        .expect("ok");
    assert_eq!(none, None);
}

/// The element of one view goes to the others as the sync agent's target: `targetOf` on the
/// node resolved into the agent's world, `find` in the other view's.
#[tokio::test]
async fn the_agent_names_the_element_and_finds_it_in_another_view() {
    let target = json!({ "element": { "kind": "testid", "testid": "menu", "path": "#m" }, "offset": { "x": 0.5, "y": 0.5 } });
    let answer = target.clone();
    let (end, router) = Replying::connect(move |method, params| match method {
        "DOM.resolveNode" => Ok(json!({ "object": { "objectId": "A-el" } })),
        "Runtime.callFunctionOn" if params["functionDeclaration"] == TARGET_OF => {
            Ok(json!({ "result": { "type": "object", "value": answer } }))
        }
        "Runtime.callFunctionOn" if params["functionDeclaration"] == FIND => {
            Ok(json!({ "result": { "type": "object", "subtype": "node", "objectId": "B-el" } }))
        }
        "DOM.describeNode" => Ok(json!({ "node": { "backendNodeId": 99 } })),
        _ => Ok(json!({})),
    });
    let named = agent_target(&router, "VIEW1", 3, 42)
        .await
        .expect("a target");
    assert_eq!(named, target);
    assert_eq!(
        end.params("DOM.resolveNode")[0],
        json!({ "backendNodeId": 42, "executionContextId": 3, "objectGroup": GROUP })
    );
    let found = agent_find(&router, "VIEW2", 5, &target).await.expect("ok");
    assert_eq!(found, Some(99));
    let call = &end.params("Runtime.callFunctionOn")[1];
    assert_eq!(call["executionContextId"], 5);
    assert_eq!(call["arguments"], json!([{ "value": target }]));
    assert_eq!(
        end.params("DOM.describeNode")[0],
        json!({ "objectId": "B-el" })
    );
    let releases = end.params("Runtime.releaseObjectGroup").len();
    assert_eq!(releases, 2, "each resolution lets its objects go");
}

#[tokio::test]
async fn a_view_without_the_element_answers_none_and_a_throwing_agent_too() {
    let (_, router) = Replying::connect(|method, _| match method {
        "Runtime.callFunctionOn" => {
            Ok(json!({ "result": { "type": "object", "subtype": "null", "value": null } }))
        }
        _ => Ok(json!({})),
    });
    let target = json!({ "element": {}, "offset": {} });
    assert_eq!(
        agent_find(&router, "V", 1, &target).await.expect("ok"),
        None
    );
    let (_, router) = Replying::connect(|method, _| match method {
        "Runtime.callFunctionOn" => Ok(json!({
            "result": { "type": "object", "objectId": "X" },
            "exceptionDetails": { "text": "boom" },
        })),
        _ => Ok(json!({})),
    });
    assert_eq!(
        agent_find(&router, "V", 1, &target).await.expect("ok"),
        None
    );
}

#[tokio::test]
async fn an_element_without_a_target_is_an_error() {
    let (_, router) = Replying::connect(|method, _| match method {
        "DOM.resolveNode" => Ok(json!({ "object": { "objectId": "A" } })),
        "Runtime.callFunctionOn" => {
            Ok(json!({ "result": { "type": "object", "subtype": "null", "value": null } }))
        }
        _ => Ok(json!({})),
    });
    let error = agent_target(&router, "V", 1, 42)
        .await
        .expect_err("no target");
    assert!(error.to_string().contains("no selector"), "{error}");
}
