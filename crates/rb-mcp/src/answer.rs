//! How tools answer: JSON text on success, an `is_error` result with a stable code on failure.

use rmcp::model::{CallToolResult, ContentBlock};
use serde::Serialize;

use crate::backend::ToolFailure;

/// What a tool method returns.
pub type ToolResult = Result<CallToolResult, rmcp::ErrorData>;

/// A success payload as pretty JSON text.
pub fn json_result(value: &impl Serialize) -> ToolResult {
    let text = serde_json::to_string_pretty(value).map_err(|error| {
        rmcp::ErrorData::internal_error(format!("the answer cannot be encoded: {error}"), None)
    })?;
    Ok(CallToolResult::success(vec![ContentBlock::text(text)]))
}

/// A failure the model can read and act on: `{"error", "code", "settingsKey"?, "legacyCode"?}`,
/// the last one the flat code a dotted one replaced, until 1.0 ([`crate::codes`]).
pub fn error_result(failure: &ToolFailure) -> CallToolResult {
    let mut body = serde_json::Map::new();
    body.insert("error".to_owned(), failure.message.clone().into());
    body.insert("code".to_owned(), failure.code.clone().into());
    if let Some(key) = &failure.settings_key {
        body.insert("settingsKey".to_owned(), key.clone().into());
    }
    crate::codes::add_legacy(&mut body);
    CallToolResult::error(vec![ContentBlock::text(
        serde_json::Value::Object(body).to_string(),
    )])
}

/// A result as a tool answer.
pub fn respond<T: Serialize>(result: Result<T, ToolFailure>) -> ToolResult {
    match result {
        Ok(value) => json_result(&value),
        Err(failure) => Ok(error_result(&failure)),
    }
}

/// A failure of a prompt or a resource (RB-095-01) as the JSON-RPC error of its request: a
/// URI or run that is not there `resources.not_found` (-32002), a bad name or argument
/// `invalid_params`, an internal one `internal_error`, every other refusal (switched off, too
/// large) `invalid_request`; the stable code and the setting in `data`, as a tool's error has
/// them (and the flat `legacyCode` until 1.0).
#[must_use]
pub fn rpc_error(failure: &ToolFailure) -> rmcp::ErrorData {
    let mut data = serde_json::Map::new();
    data.insert("code".to_owned(), failure.code.clone().into());
    if let Some(key) = &failure.settings_key {
        data.insert("settingsKey".to_owned(), key.clone().into());
    }
    crate::codes::add_legacy(&mut data);
    let message = failure.message.clone();
    let data = Some(serde_json::Value::Object(data));
    match failure.code.as_str() {
        crate::resources::NOT_FOUND
        | crate::resources::NO_FINDINGS
        | crate::resources::NO_SCREENSHOTS => rmcp::ErrorData::resource_not_found(message, data),
        crate::prompts::NOT_FOUND | crate::prompts::INVALID => {
            rmcp::ErrorData::invalid_params(message, data)
        }
        crate::codes::INTERNAL => rmcp::ErrorData::internal_error(message, data),
        _ => rmcp::ErrorData::invalid_request(message, data),
    }
}

/// `value` when its JSON has at most `max_bytes`; else `{truncated: true, bytes, preview}` with
/// the first `max_bytes` of the JSON text (cut at a character boundary).
#[must_use]
pub fn capped(value: serde_json::Value, max_bytes: usize) -> serde_json::Value {
    let text = value.to_string();
    if text.len() <= max_bytes {
        return value;
    }
    let mut end = max_bytes;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    serde_json::json!({ "truncated": true, "bytes": text.len(), "preview": &text[..end] })
}

#[cfg(test)]
#[path = "answer_tests.rs"]
mod tests;
