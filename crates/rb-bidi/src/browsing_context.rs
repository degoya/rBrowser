//! `browsingContext.*`: windows, navigation, history, the viewport, screenshots, and the events
//! that say a view navigated or went away.
//!
//! `setViewport` is sent but never awaited for long (ADR 0001, Decision 7): under fractional OS
//! scaling Firefox applies the size and never answers. Callers send it with a short grace and
//! read `innerWidth` back instead.

use serde_json::{Value, json};

use crate::error::BidiError;
use crate::protocol::Command;

/// The events of a committed navigation: a new document, a fragment, `history.pushState`.
pub const NAVIGATION_EVENTS: [&str; 3] = [
    "browsingContext.domContentLoaded",
    "browsingContext.fragmentNavigated",
    "browsingContext.historyUpdated",
];
/// A context closed (the user closed the window, or rBrowser did).
pub const CONTEXT_DESTROYED: &str = "browsingContext.contextDestroyed";

/// When `navigate` answers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wait {
    /// At once; the navigation events follow.
    None,
    /// After `DOMContentLoaded`.
    Interactive,
    /// After `load`.
    Complete,
}

/// A new top-level window.
pub fn create_window() -> Command {
    create_window_in(None)
}

/// A new top-level window in `user_context` (a named session's, RB-040-05); the default user
/// context with `None`.
pub fn create_window_in(user_context: Option<&str>) -> Command {
    match user_context {
        Some(user_context) => Command::call(
            "browsingContext.create",
            json!({ "type": "window", "userContext": user_context }),
        ),
        None => Command::call("browsingContext.create", json!({ "type": "window" })),
    }
}

/// Loads `url` in `context`.
pub fn navigate(context: &str, url: &str, wait: Wait) -> Command {
    let wait = match wait {
        Wait::None => "none",
        Wait::Interactive => "interactive",
        Wait::Complete => "complete",
    };
    Command::call(
        "browsingContext.navigate",
        json!({ "context": context, "url": url, "wait": wait }),
    )
}

/// Reloads `context`, past the cache when `ignore_cache`.
pub fn reload(context: &str, ignore_cache: bool) -> Command {
    Command::call(
        "browsingContext.reload",
        json!({ "context": context, "ignoreCache": ignore_cache, "wait": "none" }),
    )
}

/// History back (`-1`) or forward (`1`).
pub fn traverse_history(context: &str, delta: i32) -> Command {
    Command::call(
        "browsingContext.traverseHistory",
        json!({ "context": context, "delta": delta }),
    )
}

/// The viewport in CSS px and the device pixel ratio; a `dpr` of 0 or less leaves the ratio
/// to the display (Firefox refuses a ratio of 0, RB-080-38).
pub fn set_viewport(context: &str, width: u32, height: u32, dpr: f64) -> Command {
    let mut params = json!({
        "context": context,
        "viewport": { "width": width, "height": height },
    });
    if dpr > 0.0 {
        params["devicePixelRatio"] = json!(dpr);
    }
    Command::call("browsingContext.setViewport", params)
}

/// A PNG of the viewport, or of the whole document.
pub fn capture_screenshot(context: &str, full_page: bool) -> Command {
    let origin = if full_page { "document" } else { "viewport" };
    Command::call(
        "browsingContext.captureScreenshot",
        json!({ "context": context, "origin": origin }),
    )
}

/// The top-level contexts, or only `root`.
pub fn get_tree(root: Option<&str>) -> Command {
    let params = match root {
        Some(root) => json!({ "root": root, "maxDepth": 0 }),
        None => json!({ "maxDepth": 0 }),
    };
    Command::call("browsingContext.getTree", params)
}

/// Brings `context`'s window to the front.
pub fn activate(context: &str) -> Command {
    Command::call("browsingContext.activate", json!({ "context": context }))
}

/// Closes `context` without asking the page.
pub fn close(context: &str) -> Command {
    Command::call(
        "browsingContext.close",
        json!({ "context": context, "promptUnload": false }),
    )
}

/// The `context` of a `create` answer.
pub fn context_of(result: &Value) -> Result<String, BidiError> {
    result
        .get("context")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| BidiError::Protocol {
            method: "browsingContext.create".to_owned(),
            reason: "no context".to_owned(),
        })
}

/// The top-level contexts of a `getTree` answer.
pub fn top_contexts(tree: &Value) -> Vec<String> {
    entries(tree)
        .filter_map(|entry| entry.get("context").and_then(Value::as_str))
        .map(str::to_owned)
        .collect()
}

/// The client window of the first context of a `getTree` answer.
pub fn client_window(tree: &Value) -> Option<String> {
    entries(tree)
        .next()
        .and_then(|entry| entry.get("clientWindow").and_then(Value::as_str))
        .map(str::to_owned)
}

/// The PNG (base64) of a `captureScreenshot` answer.
pub fn screenshot_data(result: &Value) -> Result<String, BidiError> {
    result
        .get("data")
        .and_then(Value::as_str)
        .filter(|data| !data.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| BidiError::Protocol {
            method: "browsingContext.captureScreenshot".to_owned(),
            reason: "no image data".to_owned(),
        })
}

/// `(context, url)` of a navigation event.
pub fn navigated(method: &str, params: &Value) -> Option<(String, String)> {
    if !NAVIGATION_EVENTS.contains(&method) {
        return None;
    }
    let context = params.get("context")?.as_str()?;
    let url = params.get("url")?.as_str()?;
    Some((context.to_owned(), url.to_owned()))
}

/// The top-level context a `contextDestroyed` event closed; `None` for an iframe.
pub fn destroyed(method: &str, params: &Value) -> Option<String> {
    if method != CONTEXT_DESTROYED || params.get("parent").is_some_and(|p| !p.is_null()) {
        return None;
    }
    params
        .get("context")
        .and_then(Value::as_str)
        .map(str::to_owned)
}

fn entries(tree: &Value) -> impl Iterator<Item = &Value> {
    tree.get("contexts")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
}

#[cfg(test)]
#[path = "browsing_context_tests.rs"]
mod tests;
