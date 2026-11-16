//! The CDP calls of "Force state" (RB-090-53). States are forced on a session of rBrowser's own
//! per view ([`open_session`]), not on the view's page session: Chromium keeps forced states per
//! session's CSS agent and keys them by that session's node ids, which a `DOM.getDocument` of an
//! element capture or a check highlight on the page session would discard - and a docked
//! DevTools frontend forces on its own session, so neither undoes the other's. Detaching the
//! session ends everything it forced.
//!
//! The element of the clicked view comes from "Inspect element"'s resolver
//! ([`crate::devtools::inspect::node_at`]); the same element in another view from the sync
//! agent's selectors ([`agent_target`], [`agent_find`]: the agent's `targetOf` and `find`,
//! `agent/src/element-handle.ts`).

use std::sync::Arc;

use rb_engine::EngineError;
use serde_json::{Value, json};

use super::model::NodeChange;
use crate::cdp::CdpRouter;

/// The object group of the objects a resolution holds, released after it.
const GROUP: &str = "rbrowser-force-state";

/// The element and its ancestors up to `<html>`, across shadow roots and into a same-origin
/// parent frame: the nodes a real `:hover` reaches.
pub const CHAIN: &str = "function () {
  var chain = [];
  for (var node = this; node && chain.length < 256; ) {
    chain.push(node);
    var parent = node.parentElement;
    if (!parent) {
      var root = node.getRootNode ? node.getRootNode() : null;
      if (root && root.host) parent = root.host;
      else {
        try { parent = node.ownerDocument.defaultView.frameElement; } catch (error) { parent = null; }
      }
    }
    node = parent;
  }
  return chain;
}";

/// In the agent's world: the target of the element `this`.
pub const TARGET_OF: &str = "function () { return globalThis.__rbAgent.targetOf(this); }";
/// In the agent's world: the element a target names, or null.
pub const FIND: &str = "function (target) { return globalThis.__rbAgent.find(target); }";

fn invalid(text: &str) -> EngineError {
    EngineError::Invalid(text.to_owned())
}

/// Attaches a session of rBrowser's own to the target of the page session `page` and enables
/// DOM and CSS on it; answers the session and its document's node id.
pub async fn open_session(cdp: &Arc<CdpRouter>, page: &str) -> Result<(String, i64), EngineError> {
    let info = cdp
        .call("Target.getTargetInfo", json!({}), Some(page))
        .await?;
    let target = info
        .pointer("/targetInfo/targetId")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("the view's target has no id"))?;
    let attached = cdp
        .call(
            "Target.attachToTarget",
            json!({ "targetId": target, "flatten": true }),
            None,
        )
        .await?;
    let session = attached
        .get("sessionId")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("attachToTarget gave no session"))?
        .to_owned();
    match enable(cdp, &session).await {
        Ok(root) => Ok((session, root)),
        Err(error) => {
            detach(cdp, &session).await;
            Err(error)
        }
    }
}

async fn enable(cdp: &Arc<CdpRouter>, session: &str) -> Result<i64, EngineError> {
    cdp.call("DOM.enable", json!({}), Some(session)).await?;
    cdp.call("CSS.enable", json!({}), Some(session)).await?;
    document(cdp, session).await
}

/// The node id of the document in `session` (asked once per document: asking again discards
/// the session's node ids, and with them what it forced).
pub async fn document(cdp: &Arc<CdpRouter>, session: &str) -> Result<i64, EngineError> {
    let document = cdp
        .call("DOM.getDocument", json!({ "depth": 0 }), Some(session))
        .await?;
    document
        .pointer("/root/nodeId")
        .and_then(Value::as_i64)
        .ok_or_else(|| invalid("the page has no document"))
}

/// Ends rBrowser's session: Chromium drops what it forced.
pub async fn detach(cdp: &Arc<CdpRouter>, session: &str) {
    if let Err(error) = cdp
        .call(
            "Target.detachFromTarget",
            json!({ "sessionId": session }),
            None,
        )
        .await
    {
        tracing::debug!(%error, "force state: the session was gone already");
    }
}

/// The node ids in `session` of the element `backend` and of its ancestors ([`CHAIN`]).
pub async fn chain_of(
    cdp: &Arc<CdpRouter>,
    session: &str,
    backend: i64,
) -> Result<Vec<i64>, EngineError> {
    let chain = chain_nodes(cdp, session, backend).await;
    release(cdp, session).await;
    chain
}

async fn chain_nodes(
    cdp: &Arc<CdpRouter>,
    session: &str,
    backend: i64,
) -> Result<Vec<i64>, EngineError> {
    let on = Some(session);
    let resolved = cdp
        .call(
            "DOM.resolveNode",
            json!({ "backendNodeId": backend, "objectGroup": GROUP }),
            on,
        )
        .await?;
    let element =
        object_id(&resolved, "/object/objectId").ok_or_else(|| invalid("the element is gone"))?;
    let array = cdp
        .call(
            "Runtime.callFunctionOn",
            json!({
                "objectId": element,
                "functionDeclaration": CHAIN,
                "returnByValue": false,
                "objectGroup": GROUP,
                "silent": true,
            }),
            on,
        )
        .await?;
    let array = object_id(&array, "/result/objectId")
        .ok_or_else(|| invalid("the element's ancestors could not be read"))?;
    let properties = cdp
        .call(
            "Runtime.getProperties",
            json!({ "objectId": array, "ownProperties": true }),
            on,
        )
        .await?;
    let mut items: Vec<(usize, String)> = properties
        .get("result")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|property| {
            let index = property.get("name")?.as_str()?.parse::<usize>().ok()?;
            let object = property.pointer("/value/objectId")?.as_str()?;
            Some((index, object.to_owned()))
        })
        .collect();
    items.sort_by_key(|(index, _)| *index);
    let mut chain = Vec::with_capacity(items.len());
    for (_, object) in items {
        let node = cdp
            .call("DOM.requestNode", json!({ "objectId": object }), on)
            .await?;
        if let Some(node) = node
            .get("nodeId")
            .and_then(Value::as_i64)
            .filter(|id| *id > 0)
        {
            chain.push(node);
        }
    }
    if chain.is_empty() {
        return Err(invalid("the element has no node in the page"));
    }
    Ok(chain)
}

/// `CSS.forcePseudoState` for each change, in order.
pub async fn force(
    cdp: &Arc<CdpRouter>,
    session: &str,
    changes: &[NodeChange],
) -> Result<(), EngineError> {
    for change in changes {
        cdp.call(
            "CSS.forcePseudoState",
            json!({ "nodeId": change.node, "forcedPseudoClasses": change.classes }),
            Some(session),
        )
        .await?;
    }
    Ok(())
}

/// The backend node id of the first element `selector` matches in the document `root` of
/// `session`; `None` when none does.
pub async fn selector_node(
    cdp: &Arc<CdpRouter>,
    session: &str,
    root: i64,
    selector: &str,
) -> Result<Option<i64>, EngineError> {
    let on = Some(session);
    let found = cdp
        .call(
            "DOM.querySelector",
            json!({ "nodeId": root, "selector": selector }),
            on,
        )
        .await?;
    let Some(node) = found
        .get("nodeId")
        .and_then(Value::as_i64)
        .filter(|id| *id > 0)
    else {
        return Ok(None);
    };
    let described = cdp
        .call("DOM.describeNode", json!({ "nodeId": node }), on)
        .await?;
    Ok(described
        .pointer("/node/backendNodeId")
        .and_then(Value::as_i64))
}

/// The sync agent's target of the element `backend`, in the agent's world `context` of
/// `session` (the click sync's selectors, RB-020-01).
pub async fn agent_target(
    cdp: &Arc<CdpRouter>,
    session: &str,
    context: i64,
    backend: i64,
) -> Result<Value, EngineError> {
    let target = target_in(cdp, session, context, backend).await;
    release(cdp, session).await;
    target
}

async fn target_in(
    cdp: &Arc<CdpRouter>,
    session: &str,
    context: i64,
    backend: i64,
) -> Result<Value, EngineError> {
    let on = Some(session);
    let resolved = cdp
        .call(
            "DOM.resolveNode",
            json!({
                "backendNodeId": backend,
                "executionContextId": context,
                "objectGroup": GROUP,
            }),
            on,
        )
        .await?;
    let element =
        object_id(&resolved, "/object/objectId").ok_or_else(|| invalid("the element is gone"))?;
    let answer = cdp
        .call(
            "Runtime.callFunctionOn",
            json!({
                "objectId": element,
                "functionDeclaration": TARGET_OF,
                "returnByValue": true,
                "silent": true,
            }),
            on,
        )
        .await?;
    answer
        .pointer("/result/value")
        .filter(|value| value.is_object())
        .cloned()
        .ok_or_else(|| invalid("the sync agent has no selector for the element"))
}

/// The backend node id of the element `target` names in the agent's world `context` of
/// `session`; `None` when the page has none.
pub async fn agent_find(
    cdp: &Arc<CdpRouter>,
    session: &str,
    context: i64,
    target: &Value,
) -> Result<Option<i64>, EngineError> {
    let found = find_in(cdp, session, context, target).await;
    release(cdp, session).await;
    found
}

async fn find_in(
    cdp: &Arc<CdpRouter>,
    session: &str,
    context: i64,
    target: &Value,
) -> Result<Option<i64>, EngineError> {
    let on = Some(session);
    let answer = cdp
        .call(
            "Runtime.callFunctionOn",
            json!({
                "executionContextId": context,
                "functionDeclaration": FIND,
                "arguments": [{ "value": target }],
                "returnByValue": false,
                "objectGroup": GROUP,
                "silent": true,
            }),
            on,
        )
        .await?;
    let Some(element) = object_id(&answer, "/result/objectId") else {
        return Ok(None);
    };
    let described = cdp
        .call("DOM.describeNode", json!({ "objectId": element }), on)
        .await?;
    Ok(described
        .pointer("/node/backendNodeId")
        .and_then(Value::as_i64))
}

fn object_id<'a>(answer: &'a Value, pointer: &str) -> Option<&'a str> {
    if answer.get("exceptionDetails").is_some() {
        return None;
    }
    answer.pointer(pointer).and_then(Value::as_str)
}

async fn release(cdp: &Arc<CdpRouter>, session: &str) {
    if let Err(error) = cdp
        .call(
            "Runtime.releaseObjectGroup",
            json!({ "objectGroup": GROUP }),
            Some(session),
        )
        .await
    {
        tracing::debug!(%error, "force state: the object group stayed");
    }
}

#[cfg(test)]
#[path = "cdp_tests.rs"]
mod tests;
