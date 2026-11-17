//! Pick mode of `capture.element`: the view's inspect overlay (`Overlay.setInspectMode`,
//! `searchForNode`) until the user clicks an element, whose node then comes back from
//! `Overlay.inspectNodeRequested`. The mode ends in every case: a click, the timeout, an error.
//!
//! The overlay's hit test sees every element, rBrowser's own too: a click on a ruler's handle or
//! another plugin's host under `<rbrowser-layers>` is no pick (RB-090-54). Each node clicked is
//! checked in rBrowser's isolated world of the main frame ([`OURS`]); one of rBrowser's is passed
//! over, the mode armed again, and the next click on the page is the pick.

use std::sync::Arc;
use std::time::Duration;

use rb_engine::EngineError;
use serde_json::{Value, json};
use tokio::sync::broadcast::error::RecvError;

use crate::cdp::{CdpEvent, CdpRouter};

/// How long the overlay waits for a click.
pub const PICK_TIMEOUT: Duration = Duration::from_secs(30);
/// The isolated world the check runs in: the page sees neither the world nor the script.
pub const PICK_WORLD: &str = "rbrowser-pick";
/// The object group of the node checked, released after the check.
const PICK_GROUP: &str = "rbrowser-pick";
/// The check: whether the node (`this`) is rBrowser's own (`agent/src/pick-ours.test.ts`).
const OURS: &str = include_str!("pick_ours.js");

/// The highlight while hovering: DevTools' content blue, the size label on.
fn highlight() -> Value {
    json!({
        "showInfo": true,
        "contentColor": { "r": 111, "g": 168, "b": 220, "a": 0.66 },
        "paddingColor": { "r": 147, "g": 196, "b": 125, "a": 0.55 },
        "borderColor": { "r": 255, "g": 229, "b": 153, "a": 0.66 },
        "marginColor": { "r": 246, "g": 178, "b": 107, "a": 0.66 },
    })
}

fn search_mode() -> Value {
    json!({ "mode": "searchForNode", "highlightConfig": highlight() })
}

/// Lets the user click an element in the view of `session`; its backend node id, never one of
/// rBrowser's own layers.
pub async fn pick(
    cdp: &Arc<CdpRouter>,
    session: &str,
    timeout: Duration,
) -> Result<i64, EngineError> {
    let mut events = cdp.subscribe();
    let on = Some(session);
    cdp.call("DOM.enable", json!({}), on).await?;
    cdp.call("Overlay.enable", json!({}), on).await?;
    let started = cdp.call("Overlay.setInspectMode", search_mode(), on).await;
    let picked = match started {
        Ok(_) => wait(cdp, &mut events, session, timeout).await,
        Err(error) => Err(error),
    };
    let ended = cdp
        .call(
            "Overlay.setInspectMode",
            json!({ "mode": "none", "highlightConfig": {} }),
            on,
        )
        .await;
    if let Err(error) = ended {
        tracing::warn!(%error, "the pick overlay did not end");
    }
    if let Err(error) = cdp.call("Overlay.disable", json!({}), on).await {
        tracing::warn!(%error, "the overlay domain stayed on");
    }
    picked
}

async fn wait(
    cdp: &Arc<CdpRouter>,
    events: &mut tokio::sync::broadcast::Receiver<CdpEvent>,
    session: &str,
    timeout: Duration,
) -> Result<i64, EngineError> {
    tokio::time::timeout(timeout, clicked(cdp, events, session))
        .await
        .map_err(|_| EngineError::Invalid("no element was picked in time".to_owned()))?
}

/// The node of the first click in the view of `session` that is not on one of rBrowser's own.
async fn clicked(
    cdp: &Arc<CdpRouter>,
    events: &mut tokio::sync::broadcast::Receiver<CdpEvent>,
    session: &str,
) -> Result<i64, EngineError> {
    loop {
        match events.recv().await {
            Ok(event)
                if event.method == "Overlay.inspectNodeRequested"
                    && event.session_id.as_deref() == Some(session) =>
            {
                let Some(node) = event.params.get("backendNodeId").and_then(Value::as_i64) else {
                    continue;
                };
                if !is_ours(cdp, session, node).await {
                    return Ok(node);
                }
                // The overlay may have ended its mode with the click: on again.
                cdp.call("Overlay.setInspectMode", search_mode(), Some(session))
                    .await?;
            }
            Ok(_) | Err(RecvError::Lagged(_)) => {}
            Err(RecvError::Closed) => return Err(EngineError::Closed),
        }
    }
}

/// Whether node `backend` of the page on `session` is one of rBrowser's own ([`OURS`]). A node
/// the check cannot resolve in the main frame's world - in a child frame, gone meanwhile - is the
/// page's: rBrowser's layers live in the top document.
async fn is_ours(cdp: &Arc<CdpRouter>, session: &str, backend: i64) -> bool {
    let checked = check(cdp, session, backend).await;
    let released = cdp
        .call(
            "Runtime.releaseObjectGroup",
            json!({ "objectGroup": PICK_GROUP }),
            Some(session),
        )
        .await;
    if let Err(error) = released {
        tracing::debug!(%error, "pick: the checked node's object group stayed");
    }
    checked.unwrap_or_else(|error| {
        tracing::debug!(%error, "pick: the node clicked was not checked, taken as the page's");
        false
    })
}

async fn check(cdp: &Arc<CdpRouter>, session: &str, backend: i64) -> Result<bool, EngineError> {
    let on = Some(session);
    let tree = cdp.call("Page.getFrameTree", json!({}), on).await?;
    let frame = tree
        .pointer("/frameTree/frame/id")
        .and_then(Value::as_str)
        .ok_or_else(|| EngineError::Invalid("no main frame".to_owned()))?;
    let world = cdp
        .call(
            "Page.createIsolatedWorld",
            json!({ "frameId": frame, "worldName": PICK_WORLD }),
            on,
        )
        .await?;
    let context = world
        .get("executionContextId")
        .and_then(Value::as_i64)
        .ok_or_else(|| EngineError::Invalid("no isolated world".to_owned()))?;
    let resolved = cdp
        .call(
            "DOM.resolveNode",
            json!({ "backendNodeId": backend, "executionContextId": context, "objectGroup": PICK_GROUP }),
            on,
        )
        .await?;
    let object = resolved
        .pointer("/object/objectId")
        .and_then(Value::as_str)
        .ok_or_else(|| EngineError::Invalid("the node was not resolved".to_owned()))?;
    let answer = cdp
        .call(
            "Runtime.callFunctionOn",
            json!({
                "functionDeclaration": OURS,
                "objectId": object,
                "returnByValue": true,
                "silent": true,
            }),
            on,
        )
        .await?;
    if answer.get("exceptionDetails").is_some() {
        return Err(EngineError::Invalid("the pick check threw".to_owned()));
    }
    Ok(answer.pointer("/result/value").and_then(Value::as_bool) == Some(true))
}

#[cfg(test)]
#[path = "pick_tests.rs"]
mod tests;
