//! What `script_run` answers: the script's return value with the number of tool calls and the
//! time, cut at `mcp.scripts.maxResultBytes` with a note; or the failure with a stable code, the
//! error's name, the tool and the line where the script says so, and the setting that decides.

use std::time::Duration;

use rmcp::model::{CallToolResult, ContentBlock};
use serde_json::{Map, Value, json};

use super::{MAX_CALLS_KEY, MAX_RESULT_KEY, ScriptOptions, ScriptStop, TIMEOUT_KEY, codes};

/// How a run went besides its outcome.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ran {
    /// The tool calls that reached a tool.
    pub calls: u32,
    /// The whole run.
    pub duration: Duration,
    /// The script asked for more calls than `mcp.scripts.maxCalls`.
    pub exceeded: bool,
}

/// The answer of a run that ended with `outcome`.
#[must_use]
pub fn answer(
    outcome: Result<String, ScriptStop>,
    ran: Ran,
    options: &ScriptOptions,
) -> CallToolResult {
    if ran.exceeded {
        return failure(
            Map::new(),
            codes::CALL_LIMIT,
            &format!(
                "the script made more than {} tool calls; raise mcp.scripts.maxCalls or batch \
                 the work with target \"all\"",
                options.max_calls
            ),
            Some(MAX_CALLS_KEY),
            ran,
            options.max_result_bytes,
        );
    }
    match outcome {
        Ok(text) => success(&text, ran, options.max_result_bytes),
        Err(ScriptStop::Script(text)) => script_failure(&text, ran, options.max_result_bytes),
        Err(ScriptStop::Timeout) => failure(
            Map::new(),
            codes::TIMEOUT,
            &format!(
                "the script ran longer than {} ms (mcp.scripts.timeoutMs)",
                options.timeout.as_millis()
            ),
            Some(TIMEOUT_KEY),
            ran,
            options.max_result_bytes,
        ),
        Err(ScriptStop::OutOfMemory) => failure(
            Map::new(),
            codes::MEMORY,
            "the script took more memory than the sandbox allows",
            None,
            ran,
            options.max_result_bytes,
        ),
        Err(ScriptStop::Engine(reason)) => failure(
            Map::new(),
            codes::UNAVAILABLE,
            &format!("the script engine failed: {reason}"),
            None,
            ran,
            options.max_result_bytes,
        ),
    }
}

/// A refusal before any run: code mode off, bad arguments, no engine.
#[must_use]
pub fn refused(code: &str, message: &str, settings_key: Option<&str>) -> CallToolResult {
    let mut body = json!({ "error": message, "code": code });
    if let Some(key) = settings_key {
        body["settingsKey"] = json!(key);
    }
    CallToolResult::error(vec![ContentBlock::text(body.to_string())])
}

fn success(text: &str, ran: Ran, max_bytes: usize) -> CallToolResult {
    let answer: Value = serde_json::from_str(text).unwrap_or(Value::Null);
    let mut body = Map::new();
    body.insert(
        "result".to_owned(),
        answer.get("value").cloned().unwrap_or(Value::Null),
    );
    body.insert("calls".to_owned(), json!(ran.calls));
    body.insert("durationMs".to_owned(), json!(millis(ran.duration)));
    if let Some(logs) = answer
        .get("logs")
        .filter(|logs| logs.as_array().is_some_and(|lines| !lines.is_empty()))
    {
        body.insert("logs".to_owned(), logs.clone());
    }
    fit(&mut body, "result", true, max_bytes);
    CallToolResult::success(vec![ContentBlock::text(Value::Object(body).to_string())])
}

/// The engine's failure JSON as the answer.
fn script_failure(text: &str, ran: Ran, max_bytes: usize) -> CallToolResult {
    let Ok(Value::Object(mut fields)) = serde_json::from_str::<Value>(text) else {
        return failure(Map::new(), codes::ERROR, text, None, ran, max_bytes);
    };
    let code = fields
        .remove("code")
        .and_then(|code| code.as_str().map(str::to_owned))
        .unwrap_or_else(|| codes::ERROR.to_owned());
    let message = fields
        .remove("message")
        .and_then(|message| message.as_str().map(str::to_owned))
        .unwrap_or_default();
    if fields
        .get("logs")
        .and_then(Value::as_array)
        .is_some_and(Vec::is_empty)
    {
        fields.remove("logs");
    }
    fields.retain(|key, _| matches!(key.as_str(), "name" | "tool" | "line" | "logs"));
    let settings_key = (code == codes::CALL_LIMIT).then_some(MAX_CALLS_KEY);
    failure(fields, &code, &message, settings_key, ran, max_bytes)
}

fn failure(
    mut fields: Map<String, Value>,
    code: &str,
    message: &str,
    settings_key: Option<&str>,
    ran: Ran,
    max_bytes: usize,
) -> CallToolResult {
    fields.insert("error".to_owned(), json!(message));
    fields.insert("code".to_owned(), json!(code));
    fields.insert("calls".to_owned(), json!(ran.calls));
    fields.insert("durationMs".to_owned(), json!(millis(ran.duration)));
    if let Some(key) = settings_key {
        fields.insert("settingsKey".to_owned(), json!(key));
    }
    fit(&mut fields, "error", false, max_bytes);
    CallToolResult::error(vec![ContentBlock::text(Value::Object(fields).to_string())])
}

/// Holds `body` to `max` bytes of JSON text, measured as sent - escaping, the envelope and the
/// note included: the console lines go first; when that is not enough, `field` is cut - to
/// `{truncated, bytes, preview}` when `object`, else to its first characters with
/// `truncated: true` beside it. The note says what was dropped.
fn fit(body: &mut Map<String, Value>, field: &str, object: bool, max: usize) {
    if json_len(body) <= max {
        return;
    }
    let logs_dropped = body.remove("logs").is_some();
    if logs_dropped {
        body.insert("note".to_owned(), json!(note(None, true, max)));
        if json_len(body) <= max {
            return;
        }
    }
    let original = body.remove(field).unwrap_or(Value::Null);
    let text = match (&original, object) {
        (Value::String(text), false) => text.clone(),
        (other, _) => other.to_string(),
    };
    body.insert(
        "note".to_owned(),
        json!(note(Some(field), logs_dropped, max)),
    );
    let shaped = |preview: String| {
        if object {
            json!({ "truncated": true, "bytes": text.len(), "preview": preview })
        } else {
            Value::String(preview)
        }
    };
    body.insert(field.to_owned(), shaped(String::new()));
    if !object {
        body.insert("truncated".to_owned(), json!(true));
    }
    let preview = prefix_within(&text, max.saturating_sub(json_len(body)));
    body.insert(field.to_owned(), shaped(preview));
}

/// The longest start of `text` whose JSON-escaped form takes at most `budget` bytes.
fn prefix_within(text: &str, budget: usize) -> String {
    let mut used = 0;
    let mut prefix = String::new();
    for character in text.chars() {
        let mut buffer = [0_u8; 4];
        let escaped = serde_json::to_string(&*character.encode_utf8(&mut buffer))
            .map_or(6, |quoted| quoted.len() - 2);
        if used + escaped > budget {
            break;
        }
        used += escaped;
        prefix.push(character);
    }
    prefix
}

/// What a cut answer says about itself.
fn note(cut: Option<&str>, logs_dropped: bool, max: usize) -> String {
    match cut {
        None => format!(
            "the console lines were left out to keep the answer within {max} bytes \
             ({MAX_RESULT_KEY})"
        ),
        Some(field) => format!(
            "the {field} was cut to keep the answer within {max} bytes ({MAX_RESULT_KEY}){}: \
             return less, or filter in the script",
            if logs_dropped {
                ", and the console lines were left out"
            } else {
                ""
            }
        ),
    }
}

/// The bytes of `body` as JSON text.
fn json_len(body: &Map<String, Value>) -> usize {
    serde_json::to_string(body).map_or(usize::MAX, |text| text.len())
}

fn millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

#[cfg(test)]
#[path = "answer_tests.rs"]
mod tests;
