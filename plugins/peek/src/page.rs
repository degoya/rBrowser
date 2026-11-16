//! The page script's requests (`call-tool("@page", {"view", "message"})`, docs/plugins.md "Page
//! scripts and panels"):
//!
//! - `{"type": "config"}`: the settings, which the page script cannot read itself;
//! - `{"type": "inspect", "data": <page data>}`: the inspection of the hovered element, and -
//!   with `peek.syncHighlight` - a post to every view's page script to outline the same element
//!   (`{"type": "highlight", "from", "target"}`; the sync agent's element target, which each
//!   view resolves in its own document);
//! - `{"type": "clear"}`: the outline goes again.

use rb_plugin_api::{Failure, FailureKind, PluginHost, codes};
use serde_json::{Map, Value, json};

use crate::config::Config;
use crate::inspect::build;
use crate::node::{Extras, Locator};

/// The longest selector taken (page or MCP).
pub const MAX_SELECTOR: usize = 1024;
/// The largest element target posted to the other views, as JSON text.
pub const MAX_TARGET_BYTES: usize = 4096;
/// The largest viewport coordinate taken, in CSS px.
pub const MAX_COORDINATE: f64 = 100_000.0;
/// The largest document coordinate (a viewport point moved by the page's scroll), in CSS px;
/// beyond it the page lies.
pub const MAX_DOCUMENT: f64 = 10_000_000.0;

/// `plugins.error.invalid` with its reason.
#[must_use]
pub fn invalid(reason: &str) -> Failure {
    Failure::coded(FailureKind::BadData, codes::INVALID).with_param("reason", reason)
}

/// A coordinate of the viewport: finite, 0 to [`MAX_COORDINATE`].
pub fn coordinate(value: &Value) -> Option<f64> {
    value
        .as_f64()
        .filter(|number| number.is_finite() && (0.0..=MAX_COORDINATE).contains(number))
}

/// `viewport` (a point of the viewport) moved by `scroll` (the page's, missing is none) into the
/// document: finite and 0 to [`MAX_DOCUMENT`].
fn in_document(viewport: &Value, scroll: &Value) -> Option<f64> {
    let viewport = coordinate(viewport)?;
    let scroll = if scroll.is_null() {
        0.0
    } else {
        scroll
            .as_f64()
            .filter(|number| number.is_finite() && number.abs() <= MAX_DOCUMENT)?
    };
    Some(viewport + scroll).filter(|number| (0.0..=MAX_DOCUMENT).contains(number))
}

/// How the element of `data` is found for CDP: its `selector`, else its `point` - the viewport's,
/// moved by the page's `scroll` into the document, which `DOM.getNodeForLocation` takes
/// (RB-090-54; on a scrolled page the viewport point named an element above the pointer).
#[must_use]
pub fn locator(data: &Value) -> Option<Locator> {
    if let Some(selector) = data["selector"].as_str() {
        let length = selector.chars().count();
        return (1..=MAX_SELECTOR)
            .contains(&length)
            .then(|| Locator::Selector(selector.to_owned()));
    }
    let (point, scroll) = (&data["point"], &data["scroll"]);
    Some(Locator::Point {
        x: in_document(&point["x"], &scroll["x"])?,
        y: in_document(&point["y"], &scroll["y"])?,
    })
}

fn post(answer: &mut Map<String, Value>, message: Value) {
    answer.insert(
        "$post".to_owned(),
        json!([{ "target": "all", "message": message }]),
    );
}

/// Answers one page-script request.
///
/// # Errors
///
/// `plugins.error.invalid` for a request that is not one of the three.
pub fn handle(host: &mut dyn PluginHost, args: &str) -> Result<String, Failure> {
    let args: Value = serde_json::from_str(args).map_err(|_| invalid("arguments"))?;
    let view = args["view"]
        .as_u64()
        .and_then(|view| u32::try_from(view).ok())
        .ok_or_else(|| invalid("view"))?;
    let message = &args["message"];
    let config = Config::read(host);
    let mut answer = Map::new();
    match message["type"].as_str() {
        Some("config") => {
            answer.insert("config".to_owned(), config.to_json());
        }
        Some("inspect") => {
            let data = &message["data"];
            if !data.is_object() {
                return Err(invalid("data"));
            }
            let extras = locator(data)
                .map(|locator| Extras::read(host, view, &locator))
                .unwrap_or_default();
            answer.insert("inspection".to_owned(), build(data, &extras));
            let target = &data["target"];
            if config.sync_highlight
                && target.is_object()
                && target.to_string().len() <= MAX_TARGET_BYTES
            {
                post(
                    &mut answer,
                    json!({ "type": "highlight", "from": view, "target": target }),
                );
            }
        }
        Some("clear") => {
            if config.sync_highlight {
                post(&mut answer, json!({ "type": "clear", "from": view }));
            }
        }
        _ => return Err(invalid("message")),
    }
    Ok(Value::Object(answer).to_string())
}

#[cfg(test)]
#[path = "page_tests.rs"]
mod tests;
