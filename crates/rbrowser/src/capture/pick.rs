//! Pick mode of `capture.element`: the view's inspect overlay (`Overlay.setInspectMode`,
//! `searchForNode`) until the user clicks an element, whose node then comes back from
//! `Overlay.inspectNodeRequested`. The mode ends in every case: a click, the timeout, an error.

use std::sync::Arc;
use std::time::Duration;

use rb_engine::EngineError;
use serde_json::{Value, json};
use tokio::sync::broadcast::error::RecvError;

use crate::cdp::{CdpEvent, CdpRouter};

/// How long the overlay waits for a click.
pub const PICK_TIMEOUT: Duration = Duration::from_secs(30);

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

/// Lets the user click an element in the view of `session`; its backend node id.
pub async fn pick(
    cdp: &Arc<CdpRouter>,
    session: &str,
    timeout: Duration,
) -> Result<i64, EngineError> {
    let mut events = cdp.subscribe();
    let on = Some(session);
    cdp.call("DOM.enable", json!({}), on).await?;
    cdp.call("Overlay.enable", json!({}), on).await?;
    let started = cdp
        .call(
            "Overlay.setInspectMode",
            json!({ "mode": "searchForNode", "highlightConfig": highlight() }),
            on,
        )
        .await;
    let picked = match started {
        Ok(_) => wait(&mut events, session, timeout).await,
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
    events: &mut tokio::sync::broadcast::Receiver<CdpEvent>,
    session: &str,
    timeout: Duration,
) -> Result<i64, EngineError> {
    let clicked = async {
        loop {
            match events.recv().await {
                Ok(event)
                    if event.method == "Overlay.inspectNodeRequested"
                        && event.session_id.as_deref() == Some(session) =>
                {
                    if let Some(node) = event.params.get("backendNodeId").and_then(Value::as_i64) {
                        return Ok(node);
                    }
                }
                Ok(_) | Err(RecvError::Lagged(_)) => {}
                Err(RecvError::Closed) => return Err(EngineError::Closed),
            }
        }
    };
    tokio::time::timeout(timeout, clicked)
        .await
        .map_err(|_| EngineError::Invalid("no element was picked in time".to_owned()))?
}

#[cfg(test)]
#[path = "pick_tests.rs"]
mod tests;
