//! Requests of the list taken into the API client (RB-096-08), the capability `api-client`: the
//! plugin names the captured requests (view and the host's request id) and the core does the
//! rest - it reads their headers as sent and their post data, turns every secret into a variable
//! and writes the files below `.rbrowser/api/` (`crates/rbrowser/src/api_client/capture_ops.rs`).
//! This component never sees a header's value.
//!
//! - **`api_open`** (MCP `networkpanel_api_open`): one request; from the panel the core opens
//!   the API client with it as an unsaved draft, over MCP it only answers the request as the API
//!   client would take it.
//! - **`api_save`** (MCP `networkpanel_api_save`): up to [`MAX_REQUESTS`] requests saved, one per
//!   method and path, in a folder per host; the answer names what was saved and skipped and which
//!   variables have a value. Over MCP no value is kept.
//! - **Commands** `api_open` and `api_save` show the panel and post `{"apiOpen": true}` or
//!   `{"apiSave": true}`: the panel opens its selected request, or saves every request its
//!   filters show.

use rb_plugin_api::{Failure, FailureKind};
use serde_json::{Value, json};

use crate::{OPEN_KEY, PANEL_ID, bad_args, coded};

/// The core's offer in the arguments (the caller: `panel`, `tool`, `command`) and the op in an
/// answer.
pub const OFFER_KEY: &str = "$apiClient";
/// The op's answer in the second call's arguments.
pub const RESULT_KEY: &str = "$apiResult";
/// The most requests one save names (the core's limit).
pub const MAX_REQUESTS: usize = 200;
/// The longest request id.
const MAX_ID_CHARS: usize = 200;
/// The commands.
pub const OPEN_COMMAND: &str = "api_open";
pub const SAVE_COMMAND: &str = "api_save";

/// `{view, id}` of `args`, checked.
fn reference(args: &Value) -> Result<Value, Failure> {
    let view = args["view"]
        .as_u64()
        .and_then(|view| u32::try_from(view).ok())
        .ok_or_else(|| bad_args("view"))?;
    let id = args["id"]
        .as_str()
        .filter(|id| !id.is_empty() && id.chars().count() <= MAX_ID_CHARS)
        .ok_or_else(|| bad_args("id"))?;
    Ok(json!({ "view": view, "id": id }))
}

/// The caller the core offers, or the failure without one.
fn offer(args: &Value) -> Result<&str, Failure> {
    args[OFFER_KEY]
        .as_str()
        .ok_or_else(|| coded(FailureKind::NotSupported, "errors.no_api_client"))
}

/// `api_open`: the core's answer on the second call, else the op for request `{view, id}`.
///
/// # Errors
///
/// No API client in the core, arguments of the wrong form.
pub fn api_open(args: &Value) -> Result<Value, Failure> {
    if let Some(result) = args.get(RESULT_KEY) {
        return Ok(result.clone());
    }
    let origin = offer(args)?;
    let one = reference(args)?;
    Ok(json!({ OFFER_KEY: { "op": "capture", "requests": [one], "open": origin == "panel" } }))
}

/// `api_save`: the core's answer on the second call, else the op saving `requests`.
///
/// # Errors
///
/// No API client in the core, no list, an empty or too long one, an entry of the wrong form.
pub fn api_save(args: &Value) -> Result<Value, Failure> {
    if let Some(result) = args.get(RESULT_KEY) {
        return Ok(result.clone());
    }
    offer(args)?;
    let list = args["requests"]
        .as_array()
        .filter(|list| !list.is_empty() && list.len() <= MAX_REQUESTS)
        .ok_or_else(|| bad_args("requests"))?;
    let requests = list.iter().map(reference).collect::<Result<Vec<_>, _>>()?;
    Ok(json!({ OFFER_KEY: { "op": "capture", "requests": requests, "save": true } }))
}

/// The answer of command `name`, when it is one of these.
#[must_use]
pub fn command(name: &str) -> Option<Value> {
    let message = match name {
        OPEN_COMMAND => json!({ "apiOpen": true }),
        SAVE_COMMAND => json!({ "apiSave": true }),
        _ => return None,
    };
    Some(json!({
        OPEN_KEY: { "panel": PANEL_ID },
        "$post": [{ "target": "panel", "message": message }],
    }))
}

#[cfg(test)]
#[path = "api_handoff_tests.rs"]
mod tests;
