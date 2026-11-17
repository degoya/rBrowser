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

/// A failure the model can read and act on: `{"error", "code", "settingsKey"?}`.
pub fn error_result(failure: &ToolFailure) -> CallToolResult {
    let mut body = serde_json::json!({ "error": failure.message, "code": failure.code });
    if let Some(key) = &failure.settings_key {
        body["settingsKey"] = serde_json::Value::String(key.clone());
    }
    CallToolResult::error(vec![ContentBlock::text(body.to_string())])
}

/// A result as a tool answer.
pub fn respond<T: Serialize>(result: Result<T, ToolFailure>) -> ToolResult {
    match result {
        Ok(value) => json_result(&value),
        Err(failure) => Ok(error_result(&failure)),
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
