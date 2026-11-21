//! Marks an element in the views (the outline's click, the responsive images' hover): the element
//! scrolled into view and highlighted with DevTools' box colours, as the core's findings highlight
//! does it (`crates/rbrowser/src/checks/highlight.rs`) - over CDP `DOM` and `Overlay`, the selector
//! a parameter of `DOM.querySelector`, never script text. Once the copies of `plugins/outline` and
//! `plugins/responsive-images` (RB-094-11).

use rb_plugin_api::{Failure, FailureKind, PluginHost, ViewId, codes};
use serde_json::{Value, json};

/// The longest selector taken, in characters (the core's limit).
pub const MAX_SELECTOR_CHARS: usize = 1024;
/// The most views one mark names.
pub const MAX_TARGETS: usize = 64;

/// What to mark in one view.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Locator {
    Selector(String),
    /// A node of the accessibility tree.
    Backend(u64),
}

fn invalid(reason: &str) -> Failure {
    Failure::coded(FailureKind::BadData, codes::INVALID).with_param("reason", reason)
}

/// The targets of a `mark` call: `[{ "view", "selector" | "backendNodeId" }]`.
///
/// # Errors
///
/// `plugins.error.invalid` for a malformed list, view, selector or node id.
pub fn targets(value: &Value) -> Result<Vec<(ViewId, Locator)>, Failure> {
    let list = value.as_array().ok_or_else(|| invalid("targets"))?;
    if list.is_empty() || list.len() > MAX_TARGETS {
        return Err(invalid("targets"));
    }
    list.iter()
        .map(|entry| {
            let view = entry["view"]
                .as_u64()
                .and_then(|view| ViewId::try_from(view).ok())
                .ok_or_else(|| invalid("view"))?;
            if let Some(selector) = entry["selector"].as_str() {
                let ok = !selector.trim().is_empty()
                    && selector.chars().count() <= MAX_SELECTOR_CHARS
                    && !selector.chars().any(char::is_control);
                return if ok {
                    Ok((view, Locator::Selector(selector.to_owned())))
                } else {
                    Err(invalid("selector"))
                };
            }
            entry["backendNodeId"]
                .as_u64()
                .filter(|id| *id > 0)
                .map(|id| (view, Locator::Backend(id)))
                .ok_or_else(|| invalid("selector"))
        })
        .collect()
}

fn send(
    host: &mut dyn PluginHost,
    view: ViewId,
    method: &str,
    params: &Value,
) -> Result<Value, Failure> {
    let answer = host.cdp_send(view, method, &params.to_string())?;
    Ok(serde_json::from_str(&answer).unwrap_or(Value::Null))
}

fn node_id(value: &Value) -> Option<u64> {
    value["nodeId"].as_u64().filter(|id| *id != 0)
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

/// The front-end node id of `view`'s document. The CDP session is shared with the host and the
/// other plugins, and `DOM.getDocument` drops every node id handed out before (review finding L2
/// of Charge 7): the document is found through the root of the accessibility tree and pushed
/// into the document someone already requested (peek's `bound`), and requested only when nobody
/// has - then nobody holds a node id it could drop.
fn document(host: &mut dyn PluginHost, view: ViewId) -> Result<u64, Failure> {
    let backend = send(
        host,
        view,
        "Accessibility.getFullAXTree",
        &json!({ "depth": 1 }),
    )
    .ok()
    .and_then(|tree| tree["nodes"][0]["backendDOMNodeId"].as_u64());
    if let Some(backend) = backend {
        let params = json!({ "backendNodeIds": [backend] });
        if let Ok(pushed) = send(host, view, "DOM.pushNodesByBackendIdsToFrontend", &params)
            && let Some(id) = pushed["nodeIds"][0].as_u64().filter(|id| *id != 0)
        {
            return Ok(id);
        }
    }
    let document = send(host, view, "DOM.getDocument", &json!({ "depth": 0 }))?;
    node_id(&document["root"]).ok_or_else(|| invalid("document"))
}

/// Marks `locator` in `view` and scrolls it into view; `false` when the page has no such
/// element, whose old mark is then cleared.
///
/// # Errors
///
/// A CDP call failed (a Firefox or WebKit view, a selector the page cannot parse, the view gone).
pub fn mark(host: &mut dyn PluginHost, view: ViewId, locator: &Locator) -> Result<bool, Failure> {
    send(host, view, "DOM.enable", &json!({}))?;
    send(host, view, "Overlay.enable", &json!({}))?;
    let node = match locator {
        Locator::Selector(selector) => {
            let root = document(host, view)?;
            let found = send(
                host,
                view,
                "DOM.querySelector",
                &json!({ "nodeId": root, "selector": selector }),
            )?;
            node_id(&found).map(|id| json!({ "nodeId": id }))
        }
        Locator::Backend(id) => Some(json!({ "backendNodeId": id })),
    };
    let Some(node) = node else {
        send(host, view, "Overlay.hideHighlight", &json!({}))?;
        return Ok(false);
    };
    send(host, view, "DOM.scrollIntoViewIfNeeded", &node)?;
    let mut highlight = node;
    highlight["highlightConfig"] = highlight_config();
    send(host, view, "Overlay.highlightNode", &highlight)?;
    Ok(true)
}

/// Clears the mark in `view`.
///
/// # Errors
///
/// The CDP call failed.
pub fn clear(host: &mut dyn PluginHost, view: ViewId) -> Result<(), Failure> {
    send(host, view, "Overlay.hideHighlight", &json!({})).map(|_| ())
}

#[cfg(test)]
#[path = "mark_tests.rs"]
mod tests;
