//! What only Chromium knows about an element, over the CDP whitelist of the manifest (`DOM`,
//! `CSS`, `Accessibility`): the fonts it is really rendered with (`CSS.getPlatformFontsForNode`)
//! and its role and accessible name (`Accessibility.getPartialAXTree`).
//!
//! The element is found by selector (`DOM.querySelector` on the document the accessibility
//! tree's root names) or by a point of the document (`DOM.getNodeForLocation`, which takes
//! document px - the page script's viewport point moved by the page's scroll,
//! `crate::page::locator`); the document is requested only when nobody has, and no domain is
//! switched on (`bound`). Every step may fail - a Firefox or WebKit view has no CDP, the
//! element is gone - and a failure only leaves its part empty: the page script's heuristics
//! stand in, and the rendered font shows "n/a".
//!
//! **RB-070-13 (fonts)**: [`RenderedFont`] and [`rendered_fonts`] are the seam the font
//! integration builds on; the tool answer carries them as `font.rendered`.

use rb_plugin_api::{PluginHost, ViewId};
use serde_json::{Value, json};

/// The most rendered fonts kept per element.
pub const MAX_FONTS: usize = 16;
/// The longest role, name or font name kept, in characters.
pub const MAX_TEXT: usize = 200;

/// How the element is found.
#[derive(Clone, Debug, PartialEq)]
pub enum Locator {
    /// A CSS selector in the top document.
    Selector(String),
    /// A point of the document, in CSS pixels (`DOM.getNodeForLocation`'s; RB-090-54).
    Point { x: f64, y: f64 },
}

/// A font the element's text is rendered with.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RenderedFont {
    pub family: String,
    pub post_script_name: String,
    /// A web font, not an installed one.
    pub custom: bool,
    /// How many glyphs of the element's text it drew.
    pub glyphs: u64,
}

impl RenderedFont {
    #[must_use]
    pub fn to_json(&self) -> Value {
        json!({
            "family": self.family,
            "postScriptName": self.post_script_name,
            "custom": self.custom,
            "glyphs": self.glyphs,
        })
    }
}

/// What CDP adds to the page's data; `None` where it could not tell.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Extras {
    /// The fonts with the most glyphs first.
    pub rendered_fonts: Option<Vec<RenderedFont>>,
    pub role: Option<String>,
    pub name: Option<String>,
}

fn cut(text: &str) -> String {
    text.chars().take(MAX_TEXT).collect()
}

fn send(host: &mut dyn PluginHost, view: ViewId, method: &str, params: &Value) -> Option<Value> {
    let answer = host.cdp_send(view, method, &params.to_string()).ok()?;
    serde_json::from_str(&answer).ok()
}

/// The front-end node id of the node `backend` in `view`.
///
/// The session is shared with the host and the other plugins (review finding L6 of Charge 6):
/// `DOM.getDocument` drops every node id handed out before, so the node is pushed into the
/// document someone already requested, and the document is requested only when nobody has -
/// then nobody holds a node id it could drop.
fn bound(host: &mut dyn PluginHost, view: ViewId, backend: u64) -> Option<u64> {
    let params = json!({ "backendNodeIds": [backend] });
    let pushed = match send(host, view, "DOM.pushNodesByBackendIdsToFrontend", &params) {
        Some(pushed) => pushed,
        None => {
            send(host, view, "DOM.getDocument", &json!({ "depth": 0 }))?;
            send(host, view, "DOM.pushNodesByBackendIdsToFrontend", &params)?
        }
    };
    pushed["nodeIds"][0].as_u64().filter(|id| *id != 0)
}

/// The front-end node id of the document of `view`: the root of its accessibility tree names
/// it without requesting it again.
fn document(host: &mut dyn PluginHost, view: ViewId) -> Option<u64> {
    let tree = send(
        host,
        view,
        "Accessibility.getFullAXTree",
        &json!({ "depth": 1 }),
    )?;
    let backend = tree["nodes"].as_array()?.first()?["backendDOMNodeId"].as_u64()?;
    let params = json!({ "backendNodeIds": [backend] });
    match send(host, view, "DOM.pushNodesByBackendIdsToFrontend", &params) {
        Some(pushed) => pushed["nodeIds"][0].as_u64().filter(|id| *id != 0),
        None => {
            send(host, view, "DOM.getDocument", &json!({ "depth": 0 }))?["root"]["nodeId"].as_u64()
        }
    }
}

/// The front-end node id of the element `locator` names, if any.
fn node_id(host: &mut dyn PluginHost, view: ViewId, locator: &Locator) -> Option<u64> {
    let id = match locator {
        Locator::Selector(selector) => {
            let root = document(host, view)?;
            let params = json!({ "nodeId": root, "selector": selector });
            send(host, view, "DOM.querySelector", &params)?["nodeId"].as_u64()?
        }
        Locator::Point { x, y } => {
            // CDP takes whole pixels; `as` saturates, and the page sends finite numbers.
            let params = json!({
                "x": x.round() as i64,
                "y": y.round() as i64,
                "includeUserAgentShadowDOM": false,
            });
            let backend =
                send(host, view, "DOM.getNodeForLocation", &params)?["backendNodeId"].as_u64()?;
            bound(host, view, backend)?
        }
    };
    (id != 0).then_some(id)
}

/// The fonts node `node` is rendered with. No domain is switched on for it: the session is
/// shared, and `CSS.enable` would stay on for everyone (review finding L6 of Charge 6) - a view
/// whose CSS domain will not tell shows "n/a".
pub fn rendered_fonts(
    host: &mut dyn PluginHost,
    view: ViewId,
    node: u64,
) -> Option<Vec<RenderedFont>> {
    let params = json!({ "nodeId": node });
    let answer = send(host, view, "CSS.getPlatformFontsForNode", &params)?;
    let mut fonts: Vec<RenderedFont> = answer["fonts"]
        .as_array()?
        .iter()
        .map(|font| RenderedFont {
            family: cut(font["familyName"].as_str().unwrap_or_default()),
            post_script_name: cut(font["postScriptName"].as_str().unwrap_or_default()),
            custom: font["isCustomFont"].as_bool().unwrap_or(false),
            glyphs: font["glyphCount"].as_u64().unwrap_or(0),
        })
        .collect();
    fonts.sort_by_key(|font| std::cmp::Reverse(font.glyphs));
    fonts.truncate(MAX_FONTS);
    Some(fonts)
}

/// The computed role and accessible name of node `node`; an ignored node has neither.
fn accessibility(host: &mut dyn PluginHost, view: ViewId, node: u64) -> Option<(String, String)> {
    let params = json!({ "nodeId": node, "fetchRelatives": false });
    let answer = send(host, view, "Accessibility.getPartialAXTree", &params)?;
    let first = answer["nodes"].as_array()?.first()?;
    if first["ignored"].as_bool().unwrap_or(false) {
        return None;
    }
    let role = first["role"]["value"].as_str()?;
    let name = first["name"]["value"].as_str().unwrap_or_default();
    Some((cut(role), cut(name)))
}

impl Extras {
    /// Everything CDP tells about the element `locator` names in `view`.
    pub fn read(host: &mut dyn PluginHost, view: ViewId, locator: &Locator) -> Self {
        let Some(node) = node_id(host, view, locator) else {
            return Self::default();
        };
        let rendered_fonts = rendered_fonts(host, view, node);
        let (role, name) = accessibility(host, view, node).unzip();
        Self {
            rendered_fonts,
            role,
            name,
        }
    }
}

#[cfg(test)]
#[path = "node_tests.rs"]
mod tests;
