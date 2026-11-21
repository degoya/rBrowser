//! Load test (RB-096-07, `docs/plugins.md`): your own server put under load on purpose - many
//! virtual users sending the API client's saved requests - to find its limits and bottlenecks.
//!
//! The plugin forwards; the core does the work. Granted `load-test`, every call finds
//! `"$loadTest": "<origin>"` in its arguments; a tool answers `{"$loadTest": <op>}`, the core runs
//! the op (`crates/rbrowser/src/load_test`) and calls the tool once more with the op's answer
//! as `$loadResult`, which the tool hands back as its own. The core decides what each origin
//! may: the panel everything (a run only from the summary the person confirmed), a command only
//! Stop, MCP only the reports - it can neither start, change nor stop a run.
//!
//! - **`reports`** (MCP `load_reports`): the runs' reports, newest first.
//! - **`report`** (MCP `load_report`): one report, the newest without `id`.
//! - **`compare`** (MCP `load_compare`): two reports side by side, and what changed.
//! - **`panel`**: the panel's own calls, any op; never an MCP tool.
//! - **Commands**: `panel` (`Ctrl+Shift+Space P 5`) shows the panel; `cancel`
//!   (`Ctrl+Shift+Space K 2`, the activity bar's Stop) stops the running load test.
//!
//! The logic is plain JSON on the native traits and tested natively; `guest.rs` translates it
//! into the contract's vocabulary and exists on `wasm32` alone.

#[cfg(target_arch = "wasm32")]
mod guest;

use rb_plugin_api::feature::Feature;
use rb_plugin_api::{Failure, FailureKind, PluginHost, codes};
use serde_json::{Value, json};

/// The plugin's id: the prefix of its keys.
pub const PLUGIN_ID: &str = "load-test";
/// The panel the commands show.
pub const PANEL_ID: &str = "load-test";
/// The answer key asking the host to show a panel of the plugin.
pub const OPEN_KEY: &str = "$open";
/// The core's offer in the arguments, and the op in an answer.
pub const OFFER_KEY: &str = "$loadTest";
/// The op's answer in the second call's arguments.
pub const RESULT_KEY: &str = "$loadResult";

/// The tools.
pub const REPORTS: &str = "reports";
pub const REPORT: &str = "report";
pub const COMPARE: &str = "compare";
pub const PANEL_TOOL: &str = "panel";
/// The commands.
pub const PANEL: &str = "panel";
pub const CANCEL: &str = "cancel";

/// The i18n key `plugin.load-test.<name>`.
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

fn no_core() -> Failure {
    Failure::coded(FailureKind::NotSupported, &key("errors.no_core"))
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
        REPORTS => json!({ "op": "reports" }),
        REPORT => json!({ "op": "report", "id": text("id")? }),
        COMPARE => {
            let a = text("a")?.ok_or_else(|| bad_args("a"))?;
            let b = text("b")?.ok_or_else(|| bad_args("b"))?;
            json!({ "op": "compare", "a": a, "b": b })
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
        return Err(no_core());
    }
    Ok(json!({ OFFER_KEY: op_of(tool, args)? }))
}

/// A command's answer: `panel` shows the panel; `cancel` asks the core to stop the run and,
/// once stopped, tells the panel.
///
/// # Errors
///
/// An unknown command; `cancel` without the core.
pub fn command_answer(command: &str, args: &Value) -> Result<Value, Failure> {
    match command {
        PANEL => Ok(json!({ OPEN_KEY: { "panel": PANEL_ID } })),
        CANCEL => {
            if let Some(result) = args.get(RESULT_KEY) {
                return Ok(json!({
                    "$post": [{ "target": "panel", "message": { "stopped": result } }],
                }));
            }
            if args.get(OFFER_KEY).is_none() {
                return Err(no_core());
            }
            Ok(json!({ OFFER_KEY: { "op": "stop" } }))
        }
        other => Err(unknown(other)),
    }
}

/// The plugin.
pub struct LoadTest;

impl Feature for LoadTest {
    fn run_command(
        &mut self,
        _host: &mut dyn PluginHost,
        command: &str,
        args: &str,
    ) -> Result<String, Failure> {
        let args = parse(args)?;
        command_answer(command, &args).map(|value| value.to_string())
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
