//! `status_messages` (RB-080-15), run by the host (`rb_mcp::Backend::host_tool`): the status
//! bar's last messages, newest first, each with its level, time, i18n key and English text. A
//! read only. A message may carry the CDP endpoint with its token: each text goes through
//! `rb_diag::redact::redact`, which masks the registered secrets (the CDP, MCP and portal
//! tokens) - `rb-mcp`'s own masking of the answer only knows patterns.

use rb_ipc::NotificationLevel;
use rb_mcp::ToolFailure;
use serde::Deserialize;
use serde_json::{Map, Value, json};

use super::StatusLog;
use crate::control::Core;

/// The host tool's name.
pub const STATUS_MESSAGES_TOOL: &str = "status_messages";
/// How many messages an answer holds when the call names no limit.
pub const DEFAULT_LIMIT: usize = 50;
/// The most messages one answer holds.
pub const MAX_LIMIT: usize = 1000;

/// The tool's arguments.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Args {
    #[serde(default)]
    limit: Option<usize>,
    #[serde(default)]
    level: Option<NotificationLevel>,
}

/// Answers `status_messages` from `core`'s log.
///
/// # Errors
///
/// `args.invalid` for a limit outside 1..=[`MAX_LIMIT`] or an unknown level.
pub fn status_messages(core: &Core, args: &Value) -> Result<Value, ToolFailure> {
    answer(core.status_log(), args)
}

fn answer(log: &StatusLog, args: &Value) -> Result<Value, ToolFailure> {
    let args: Args = if args.is_null() {
        Args::default()
    } else {
        serde_json::from_value(args.clone())
            .map_err(|error| ToolFailure::new("args.invalid", error.to_string()))?
    };
    let limit = args.limit.unwrap_or(DEFAULT_LIMIT);
    if limit == 0 || limit > MAX_LIMIT {
        return Err(ToolFailure::new(
            "args.invalid",
            format!("`limit` must be 1 to {MAX_LIMIT}"),
        ));
    }
    let messages: Vec<Value> = log
        .recent(limit, args.level)
        .into_iter()
        .map(|message| {
            let placeholders: Vec<(&str, &str)> = message
                .args
                .iter()
                .map(|(name, value)| (name.as_str(), value.as_str()))
                .collect();
            let mut row = Map::new();
            row.insert("id".to_owned(), json!(message.id));
            row.insert("at".to_owned(), json!(message.at));
            row.insert("level".to_owned(), json!(message.level));
            row.insert("messageKey".to_owned(), json!(message.message_key));
            row.insert(
                "text".to_owned(),
                json!(rb_diag::redact::redact(&rb_i18n::t_with(
                    &message.message_key,
                    &placeholders
                ))),
            );
            if let Some(key) = &message.settings_key {
                row.insert("settingsKey".to_owned(), json!(key));
            }
            Value::Object(row)
        })
        .collect();
    Ok(json!({ "messages": messages, "kept": log.items().len() }))
}

#[cfg(test)]
#[path = "mcp_tests.rs"]
mod tests;
