//! `highlightFinding` (RB-060-01): marks a finding's element in the views the panel names and
//! scrolls it into view, over CDP (`Overlay.highlightNode`).
//!
//! The selector comes from a check plugin, so it is untrusted: [`validate`] holds it to
//! [`MAX_SELECTOR_CHARS`] without control characters, and [`highlight_view`] only hands it to
//! `DOM.querySelector` as a parameter - never to `Runtime.evaluate`, never into script text. A
//! view the window does not have is refused before anything is sent.

use async_trait::async_trait;
use rb_engine::{EngineError, ViewHost, ViewId};
use rb_ipc::IpcErrorCode;
use serde_json::{Value, json};

use crate::control::Core;
use crate::host::Host;
use crate::ipc::Failure;

/// The longest selector taken, in characters.
pub const MAX_SELECTOR_CHARS: usize = 1024;
/// The longest finding id taken (a fingerprint is 16 hex digits).
pub const MAX_ID_CHARS: usize = 128;
/// The most views one request names.
pub const MAX_VIEWS: usize = 64;

/// Why a highlight request is refused.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum HighlightError {
    #[error(
        "the selector is empty, longer than {max} characters or holds control characters",
        max = MAX_SELECTOR_CHARS
    )]
    Selector,
    #[error("the finding id is malformed")]
    FindingId,
    #[error("no view, or more than {max}", max = MAX_VIEWS)]
    Views,
    #[error("view {0} is not in the window")]
    UnknownView(ViewId),
}

/// Checks a request: the finding id, the selector, and `views` - each one of `known`.
///
/// # Errors
///
/// The first part that does not hold.
pub fn validate(
    finding_id: &str,
    selector: &str,
    views: &[ViewId],
    known: &[ViewId],
) -> Result<(), HighlightError> {
    let id_ok = !finding_id.is_empty()
        && finding_id.chars().count() <= MAX_ID_CHARS
        && finding_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | ':'));
    if !id_ok {
        return Err(HighlightError::FindingId);
    }
    let selector_ok = !selector.trim().is_empty()
        && selector.chars().count() <= MAX_SELECTOR_CHARS
        && !selector.chars().any(char::is_control);
    if !selector_ok {
        return Err(HighlightError::Selector);
    }
    if views.is_empty() || views.len() > MAX_VIEWS {
        return Err(HighlightError::Views);
    }
    match views.iter().find(|view| !known.contains(view)) {
        Some(view) => Err(HighlightError::UnknownView(*view)),
        None => Ok(()),
    }
}

/// A view's CDP session, as the highlight needs it (the host; a fake in the tests).
#[async_trait]
pub trait ViewCdp: Send + Sync {
    /// Sends `method` to `view`'s target and returns its `result`.
    async fn send(&self, view: ViewId, method: &str, params: Value) -> Result<Value, EngineError>;
}

#[async_trait]
impl ViewCdp for Host {
    async fn send(&self, view: ViewId, method: &str, params: Value) -> Result<Value, EngineError> {
        self.cdp_send(view, method, params).await
    }
}

/// DevTools' colours of content, padding, border and margin, with the size label.
fn highlight_config() -> Value {
    json!({
        "showInfo": true,
        "contentColor": { "r": 111, "g": 168, "b": 220, "a": 0.66 },
        "paddingColor": { "r": 147, "g": 196, "b": 125, "a": 0.55 },
        "borderColor": { "r": 255, "g": 229, "b": 153, "a": 0.66 },
        "marginColor": { "r": 246, "g": 178, "b": 107, "a": 0.66 }
    })
}

/// A node id of a DOM answer; `None` for none or `0` (nothing found).
fn node_id(value: &Value) -> Option<u64> {
    value
        .get("nodeId")
        .and_then(Value::as_u64)
        .filter(|id| *id != 0)
}

/// Marks the element `selector` finds in `view` and scrolls it into view; `false` when the
/// page has none, whose old mark is then cleared.
///
/// # Errors
///
/// A CDP call failed (no session yet, a selector the page cannot parse, the view closed).
pub async fn highlight_view(
    cdp: &dyn ViewCdp,
    view: ViewId,
    selector: &str,
) -> Result<bool, EngineError> {
    // The overlay needs the DOM domain; enabling twice is harmless.
    cdp.send(view, "DOM.enable", json!({})).await?;
    cdp.send(view, "Overlay.enable", json!({})).await?;
    let document = cdp
        .send(view, "DOM.getDocument", json!({ "depth": 0 }))
        .await?;
    let root = document
        .get("root")
        .and_then(node_id)
        .ok_or_else(|| EngineError::Invalid("DOM.getDocument gave no document".to_owned()))?;
    let found = cdp
        .send(
            view,
            "DOM.querySelector",
            json!({ "nodeId": root, "selector": selector }),
        )
        .await?;
    let Some(node) = node_id(&found) else {
        cdp.send(view, "Overlay.hideHighlight", json!({})).await?;
        return Ok(false);
    };
    cdp.send(
        view,
        "DOM.scrollIntoViewIfNeeded",
        json!({ "nodeId": node }),
    )
    .await?;
    cdp.send(
        view,
        "Overlay.highlightNode",
        json!({ "nodeId": node, "highlightConfig": highlight_config() }),
    )
    .await?;
    Ok(true)
}

/// Marks the element in every view of `views` at once; each view's outcome, in that order.
pub async fn highlight_all(
    cdp: &dyn ViewCdp,
    views: &[ViewId],
    selector: &str,
) -> Vec<(ViewId, Result<bool, EngineError>)> {
    futures_util::future::join_all(
        views
            .iter()
            .map(|view| async move { (*view, highlight_view(cdp, *view, selector).await) }),
    )
    .await
}

/// `highlightFinding`: checked here, the CDP calls on the runtime (answered at once).
pub(super) fn start(
    core: &Core,
    finding_id: &str,
    selector: &str,
    views: &[ViewId],
) -> Result<(), Failure> {
    let host = core.host().clone();
    let known: Vec<ViewId> = host.views().iter().map(|view| view.id).collect();
    validate(finding_id, selector, views, &known)
        .map_err(|error| (IpcErrorCode::BadRequest, error.to_string()))?;
    let (selector, views) = (selector.to_owned(), views.to_vec());
    core.runtime().spawn(async move {
        for (view, outcome) in highlight_all(&*host, &views, &selector).await {
            match outcome {
                Ok(true) => {}
                Ok(false) => tracing::debug!(%view, "the finding's element is not in the view"),
                // The selector is a plugin's text: not logged.
                Err(error) => tracing::debug!(%view, %error, "the finding was not marked"),
            }
        }
    });
    Ok(())
}

#[cfg(test)]
#[path = "highlight_tests.rs"]
mod tests;
