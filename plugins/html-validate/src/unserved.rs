//! Why a view holds no served source, in words a person can act on (RB-099-06), and the one
//! place the source can still come from without a second request.
//!
//! `Page.getResourceContent` answers the main document only while the browser keeps it; it fails
//! with CDP's words ("Content unavailable. Resource was not cached") for a page sent with
//! `Cache-Control: no-store`, a sign-in answer (401, 407) or a document the browser dropped.
//! Then [`explain`] asks the page itself, in the plugin's isolated world ([`STATE_SCRIPT`]): the
//! main document's HTTP status and whether it is still loading. A sign-in answer says so (and its
//! body is never validated in the page's place); a page still loading says so. Otherwise the
//! view's own network log may still hold the body: `Network.getResponseBody` of the document's
//! request, whose id is the frame's `loaderId` - read from the view's memory, never a second
//! request to the server, and only while the view's network requests are collected (the setting
//! `mcp.collectNetwork`, or a first read of them), as the browser keeps a body for that log
//! alone. What still has no source is `errors.not_kept` (reload, the setting, or the rendered
//! DOM) or, for another reason, `errors.no_source` with the browser's words as the detail.

use rb_plugin_api::{Failure, FailureKind, PluginHost, ViewId};
use serde_json::{Value, json};

use crate::key;
use crate::source::{no_source, reason};

/// The probe: the main document's HTTP status (0 when the browser does not say) and its ready
/// state - one expression, nothing left in the world's globals.
pub const STATE_SCRIPT: &str = "(() => { const n = performance.getEntriesByType('navigation')[0]; \
     return { status: n && typeof n.responseStatus === 'number' ? n.responseStatus : 0, \
     ready: document.readyState }; })()";

/// What the page in a view says about its load.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PageState {
    /// The main document's HTTP status; 0 when unknown.
    pub status: u16,
    /// Whether the document is still loading.
    pub loading: bool,
}

/// Why a view has no served source.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Why {
    /// The server or a proxy answered with a sign-in request (the status, 401 or 407).
    SignIn(u16),
    /// The page has not finished loading.
    NotLoaded,
    /// The browser did not keep the document.
    NotKept,
    /// Anything else: the browser's words are the detail.
    Other,
}

/// The browser's words for a document it does not hold.
const NOT_KEPT_WORDS: &[&str] = &[
    "not cached",
    "content unavailable",
    "no resource with given url",
];

/// Why there is no source, from what the page says (`state`, when the probe answered) and the
/// browser's `reason`.
#[must_use]
pub fn classify(state: Option<PageState>, reason: &str) -> Why {
    if let Some(state) = state {
        if matches!(state.status, 401 | 407) {
            return Why::SignIn(state.status);
        }
        if state.loading {
            return Why::NotLoaded;
        }
    }
    let lower = reason.to_ascii_lowercase();
    if NOT_KEPT_WORDS.iter().any(|words| lower.contains(words)) {
        Why::NotKept
    } else {
        Why::Other
    }
}

/// The probe's answer: an object, or the same as a JSON string; `None` when it does not read.
#[must_use]
pub fn read_state(answer: &str) -> Option<PageState> {
    let value: Value = serde_json::from_str(answer).ok()?;
    let value = match value {
        Value::String(inner) => serde_json::from_str(&inner).ok()?,
        other => other,
    };
    if !value.is_object() {
        return None;
    }
    let status = value["status"]
        .as_u64()
        .and_then(|status| u16::try_from(status).ok())
        .unwrap_or(0);
    Some(PageState {
        status,
        loading: value["ready"].as_str() == Some("loading"),
    })
}

/// What the page in `view` says about its load; `None` when the probe fails.
fn page_state(host: &mut dyn PluginHost, view: ViewId) -> Option<PageState> {
    host.evaluate(view, STATE_SCRIPT)
        .ok()
        .and_then(|answer| read_state(&answer))
}

/// The document's body as the view's network log holds it: text only.
fn kept_body(host: &mut dyn PluginHost, view: ViewId, loader: &str) -> Option<String> {
    let params = json!({ "requestId": loader }).to_string();
    let answer = host
        .cdp_send(view, "Network.getResponseBody", &params)
        .ok()?;
    let value: Value = serde_json::from_str(&answer).ok()?;
    if value["base64Encoded"].as_bool().unwrap_or(false) {
        return None;
    }
    value["body"].as_str().map(str::to_owned)
}

fn coded(name: &str, kind: FailureKind, view: ViewId) -> Failure {
    Failure::coded(kind, &key("errors", name)).with_param("view", view)
}

/// The served source of `view` after `Page.getResourceContent` failed with `failure`: the body
/// the view's network log holds under `loader` (the main frame's `loaderId`), or why there is
/// none.
///
/// # Errors
///
/// `errors.sign_in` (with the `status`), `errors.not_loaded`, `errors.not_kept` (with the
/// browser's `reason`) or `errors.no_source` (with it) - see [`classify`].
pub fn explain(
    host: &mut dyn PluginHost,
    view: ViewId,
    loader: Option<&str>,
    failure: &Failure,
) -> Result<String, Failure> {
    let reason = reason(failure);
    let state = page_state(host, view);
    let why = classify(state, &reason);
    match why {
        Why::SignIn(status) => {
            return Err(
                coded("sign_in", FailureKind::AccessDenied, view).with_param("status", status)
            );
        }
        Why::NotLoaded => return Err(coded("not_loaded", FailureKind::NotFound, view)),
        Why::NotKept | Why::Other => {}
    }
    // Only the body of a page known to have succeeded: without the probe it may be an error or
    // sign-in page, validated as if it were the site's.
    let succeeded = state.is_some_and(|state| (200..300).contains(&state.status));
    if let Some(body) = loader
        .filter(|_| succeeded)
        .and_then(|loader| kept_body(host, view, loader))
    {
        return Ok(body);
    }
    if why == Why::NotKept {
        Err(coded("not_kept", FailureKind::NotFound, view).with_param("reason", reason))
    } else {
        Err(no_source(view, &reason))
    }
}

#[cfg(test)]
#[path = "unserved_tests.rs"]
mod tests;
