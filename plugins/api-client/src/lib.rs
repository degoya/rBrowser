//! API client (RB-096-01, `docs/plugins.md`): HTTP APIs tested in a panel, the collections kept
//! as one `.http` file per request below `.rbrowser/api/`.
//!
//! The plugin forwards; the core does the work. Granted `api-client`, every call finds
//! `"$apiClient": "<origin>"` in its arguments; a tool answers `{"$apiClient": <op>}`, the core
//! runs the op (`crates/rbrowser/src/api_client`) and calls the tool once more with the op's
//! answer as `$apiResult`, which the tool hands back as its own. The core decides what each
//! origin may: the panel everything, MCP and the commands only reading and running saved
//! requests.
//!
//! - **`collections`** (MCP `api_collections`): the requests, or one with `id`.
//! - **`run`** (MCP `api_run`): a saved request sent in the environment the views show (or
//!   `environment`), its answer with every secret masked.
//! - **`history`** (MCP `api_history`): the last sends.
//! - **`run_collection`** (MCP `api_run_collection`, RB-096-04): every saved request of a folder
//!   (all without one) sent and judged by its checks; the report, its failures as findings.
//! - **`report`** (MCP `api_report`): the last collection run's report.
//! - **`bench`** (MCP `api_bench`): a saved request sent a few times in one or two
//!   environments, its phases as min, median, p95 and max.
//! - **`code`** (MCP `api_code`, RB-096-03): a saved request as curl, HTTPie, `fetch` or a
//!   Playwright test, the environment's plain values in, every secret a variable.
//! - **`panel`**: the panel's own calls, any op; never an MCP tool.
//! - **Commands**: `panel` (`Ctrl+Shift+Space P 4`), `send` (`Ctrl+Shift+Space D Y`, the
//!   panel's request sent) and `run_collection` (`Ctrl+Shift+Space K 1`, the panel runs every
//!   saved request).
//!
//! The logic is plain JSON on the native traits and tested natively; `guest.rs` translates it
//! into the contract's vocabulary and exists on `wasm32` alone.

#[cfg(target_arch = "wasm32")]
mod guest;

use rb_plugin_api::feature::Feature;
use rb_plugin_api::{Failure, FailureKind, PluginHost, codes};
use serde_json::{Value, json};

/// The plugin's id: the prefix of its keys.
pub const PLUGIN_ID: &str = "api-client";
/// The panel the commands show.
pub const PANEL_ID: &str = "api-client";
/// The answer key asking the host to show a panel of the plugin.
pub const OPEN_KEY: &str = "$open";
/// The core's offer in the arguments, and the op in an answer.
pub const OFFER_KEY: &str = "$apiClient";
/// The op's answer in the second call's arguments.
pub const RESULT_KEY: &str = "$apiResult";

/// The tools.
pub const COLLECTIONS: &str = "collections";
pub const RUN: &str = "run";
pub const HISTORY: &str = "history";
pub const RUN_COLLECTION: &str = "run_collection";
pub const REPORT: &str = "report";
pub const BENCH: &str = "bench";
pub const CODE: &str = "code";
pub const PANEL_TOOL: &str = "panel";
/// The commands.
pub const PANEL: &str = "panel";
pub const SEND: &str = "send";
pub const RUN_COLLECTION_COMMAND: &str = "run_collection";

/// The i18n key `plugin.api-client.<name>`.
#[must_use]
pub fn key(name: &str) -> String {
    format!("plugin.{PLUGIN_ID}.{name}")
}

fn bad_args(reason: &str) -> Failure {
    Failure::coded(FailureKind::BadData, &key("errors.bad_args")).with_param("reason", reason)
}

fn unknown(name: &str) -> Failure {
    let name: String = name.chars().take(64).collect();
    Failure::coded(FailureKind::NotFound, codes::UNKNOWN_CONTRIBUTION).with_param("name", name)
}

/// `args` as a JSON object (`{}` for an empty text).
///
/// # Errors
///
/// No JSON object.
pub fn parse(args: &str) -> Result<Value, Failure> {
    if args.trim().is_empty() {
        return Ok(json!({}));
    }
    match serde_json::from_str::<Value>(args) {
        Ok(value) if value.is_object() => Ok(value),
        _ => Err(bad_args("arguments")),
    }
}

/// The op tool `tool` asks the core for, from its `args`.
///
/// # Errors
///
/// Arguments of the wrong form, an unknown tool.
pub fn op_of(tool: &str, args: &Value) -> Result<Value, Failure> {
    let text = |name: &str| -> Result<Option<String>, Failure> {
        match &args[name] {
            Value::Null => Ok(None),
            Value::String(text) => Ok(Some(text.clone())),
            _ => Err(bad_args(name)),
        }
    };
    Ok(match tool {
        COLLECTIONS => match text("id")? {
            Some(id) => json!({ "op": "load", "id": id }),
            None => json!({ "op": "list" }),
        },
        RUN => {
            let id = text("id")?.ok_or_else(|| bad_args("id"))?;
            json!({ "op": "run", "id": id, "environment": text("environment")? })
        }
        HISTORY => json!({ "op": "history" }),
        RUN_COLLECTION => json!({
            "op": "runCollection",
            "folder": text("folder")?,
            "environment": text("environment")?,
        }),
        REPORT => json!({ "op": "report" }),
        BENCH => {
            let id = text("id")?.ok_or_else(|| bad_args("id"))?;
            let environments: Vec<String> = match &args["environments"] {
                Value::Null => Vec::new(),
                Value::Array(items) => items
                    .iter()
                    .map(|item| {
                        item.as_str()
                            .map(str::to_owned)
                            .ok_or_else(|| bad_args("environments"))
                    })
                    .collect::<Result<_, _>>()?,
                _ => return Err(bad_args("environments")),
            };
            json!({ "op": "bench", "id": id, "environments": environments })
        }
        CODE => {
            let id = text("id")?.ok_or_else(|| bad_args("id"))?;
            let language = text("language")?.ok_or_else(|| bad_args("language"))?;
            json!({ "op": "code", "id": id, "language": language, "environment": text("environment")? })
        }
        PANEL_TOOL => match &args["op"] {
            Value::Object(op) => Value::Object(op.clone()),
            _ => return Err(bad_args("op")),
        },
        other => return Err(unknown(other)),
    })
}

/// A tool's answer: the core's result on the second call, else the op to ask for.
///
/// # Errors
///
/// No offer (the core does not serve the plugin), or [`op_of`]'s.
pub fn tool_answer(tool: &str, args: &Value) -> Result<Value, Failure> {
    if let Some(result) = args.get(RESULT_KEY) {
        return Ok(result.clone());
    }
    if args.get(OFFER_KEY).is_none() {
        return Err(Failure::coded(
            FailureKind::NotSupported,
            &key("errors.no_core"),
        ));
    }
    Ok(json!({ OFFER_KEY: op_of(tool, args)? }))
}

/// A command's answer: the panel shown; `send` also has it send its request, `run_collection`
/// run every saved request.
///
/// # Errors
///
/// An unknown command.
pub fn command_answer(command: &str) -> Result<Value, Failure> {
    match command {
        PANEL => Ok(json!({ OPEN_KEY: { "panel": PANEL_ID } })),
        SEND => Ok(json!({
            OPEN_KEY: { "panel": PANEL_ID },
            "$post": [{ "target": "panel", "message": { "send": true } }],
        })),
        RUN_COLLECTION_COMMAND => Ok(json!({
            OPEN_KEY: { "panel": PANEL_ID },
            "$post": [{ "target": "panel", "message": { "runCollection": true } }],
        })),
        other => Err(unknown(other)),
    }
}

/// The plugin.
pub struct ApiClient;

impl Feature for ApiClient {
    fn run_command(
        &mut self,
        _host: &mut dyn PluginHost,
        command: &str,
        args: &str,
    ) -> Result<String, Failure> {
        parse(args)?;
        command_answer(command).map(|value| value.to_string())
    }

    fn call_tool(
        &mut self,
        _host: &mut dyn PluginHost,
        tool: &str,
        args: &str,
    ) -> Result<String, Failure> {
        let args = parse(args)?;
        tool_answer(tool, &args).map(|value| value.to_string())
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
