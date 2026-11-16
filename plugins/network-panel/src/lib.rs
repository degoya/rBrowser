//! The network panel (RB-070-02, `docs/plugins.md`): the requests of every view in one list.
//!
//! - **`list`** (MCP `networkpanel_list`, the panel): the host's capture grouped by method and URL,
//!   marked `partial` (not every view loaded it) or `differs` ([`requests`]).
//! - **Mocks, blocking, rewrites** ([`mocks`]): `.rbrowser/mocks/*.json` and the plugin's own
//!   files, under workspace trust; the person's block list (setting `block`) always. The host
//!   asks for them (`@rules`, [`rules`]) and applies them through CDP Fetch; this component never
//!   sees a request in flight. `mock_add`, `mock_remove`, `mock_enable`, `mocks` ([`edit`]).
//! - **HAR** export and import ([`har`], [`har_import`]).
//! - **The session's switches** ([`store`]): mocks on/off, the cache off, "clear site data".
//! - **The HTTP debugger** (RB-080-37, [`debug`]): breakpoints that hold a request or a
//!   response for editing, continue and abort (for one view or all), compose and repeat from a
//!   view's context; the rule kinds `mapLocal` and `mapRemote` and the extended rewrite
//!   ([`kinds`]); sessions saved as HAR and opened for display ([`sessions`]).
//!
//! The logic is plain JSON on the native traits and tested natively; `guest.rs` translates it
//! into the contract's vocabulary and exists on `wasm32` alone.

#[cfg(target_arch = "wasm32")]
mod guest;

pub mod context;
pub mod debug;
pub mod edit;
pub mod har;
pub mod har_import;
pub mod kinds;
pub mod mocks;
pub mod requests;
pub mod rules;
pub mod sessions;
pub mod store;
pub mod tools;

#[cfg(test)]
mod fake_host;

use rb_plugin_api::feature::Feature;
use rb_plugin_api::{Failure, FailureKind, PluginHost, codes};
use serde_json::{Value, json};

/// The plugin's id: the prefix of its keys.
pub const PLUGIN_ID: &str = "network-panel";
/// The panel the command `panel` shows.
pub const PANEL_ID: &str = "network";
/// The answer key asking the host to show a panel of the plugin.
pub const OPEN_KEY: &str = "$open";
/// The tool the host asks for the rules with (never a manifest tool).
pub const RULES_TOOL: &str = "@rules";

/// The i18n key `plugin.network-panel.<name>`.
#[must_use]
pub fn key(name: &str) -> String {
    format!("plugin.{PLUGIN_ID}.{name}")
}

pub(crate) fn coded(kind: FailureKind, name: &str) -> Failure {
    Failure::coded(kind, &key(name))
}

pub(crate) fn bad_args(reason: &str) -> Failure {
    coded(FailureKind::BadData, "errors.bad_args")
        .with_param("reason", reason.chars().take(200).collect::<String>())
}

fn unknown(name: &str) -> Failure {
    Failure::coded(FailureKind::NotFound, codes::UNKNOWN_CONTRIBUTION)
        .with_param("name", name.chars().take(64).collect::<String>())
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

/// The plugin.
pub struct NetworkPanel;

fn answer(value: Result<Value, Failure>) -> Result<String, Failure> {
    value.map(|value| value.to_string())
}

impl Feature for NetworkPanel {
    fn run_command(
        &mut self,
        host: &mut dyn PluginHost,
        command: &str,
        args: &str,
    ) -> Result<String, Failure> {
        let args = parse(args)?;
        match command {
            "panel" => answer(Ok(json!({ OPEN_KEY: { "panel": PANEL_ID } }))),
            "mocks_toggle" => answer(tools::mocks_toggle(host, &args)),
            "cache_toggle" => answer(tools::cache_toggle(host, &args)),
            "har_export" => answer(tools::har_export(host, &args)),
            "site_data_clear" => answer(tools::site_data_clear(host, &args)),
            "breakpoints_toggle" => answer(debug::breakpoints_toggle(host, &args)),
            other => Err(unknown(other)),
        }
    }

    fn call_tool(
        &mut self,
        host: &mut dyn PluginHost,
        tool: &str,
        args: &str,
    ) -> Result<String, Failure> {
        let args = parse(args)?;
        match tool {
            RULES_TOOL => answer(Ok(rules::document(host, &context::Context::read(&args)))),
            "list" => answer(tools::list(host, &args)),
            "body" => answer(tools::body(host, &args)),
            "mocks" => answer(Ok(edit::overview(host, &args))),
            "mock_add" => answer(edit::add(host, &args)),
            "mock_remove" => answer(edit::remove(host, &args)),
            "mock_enable" => answer(edit::enable(host, &args)),
            "mocks_toggle" => answer(tools::mocks_toggle(host, &args)),
            "cache_toggle" => answer(tools::cache_toggle(host, &args)),
            "har_export" => answer(tools::har_export(host, &args)),
            "har_import" => answer(tools::har_import(host, &args)),
            "debug" => answer(Ok(debug::state(host, &args))),
            "breakpoint_add" => answer(debug::breakpoint_add(host, &args)),
            "breakpoint_remove" => answer(debug::breakpoint_remove(host, &args)),
            "breakpoint_enable" => answer(debug::breakpoint_enable(host, &args)),
            "breakpoints_toggle" => answer(debug::breakpoints_toggle(host, &args)),
            "continue" => answer(debug::continue_held(host, &args)),
            "abort" => answer(debug::abort(host, &args)),
            "compose" => answer(debug::compose(host, &args)),
            "repeat" => answer(debug::repeat(host, &args)),
            "session_save" => answer(sessions::save(host, &args)),
            "session_open" => answer(sessions::open(host, &args)),
            "sessions_list" => answer(Ok(sessions::list(host))),
            other => Err(unknown(other)),
        }
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
