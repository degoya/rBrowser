//! The storage panel (RB-070-01, `docs/plugins.md`): cookies, localStorage and sessionStorage
//! of every view side by side.
//!
//! - **The panel** `storage` (command `panel`, `Ctrl+Shift+Space P A`; its calls in [`panel`]):
//!   one tab per kind, a row per cookie or key, a column per view grouped by session
//!   ([`columns`]), cells that differ or are missing marked ([`matrix`]); edit, add, delete in
//!   one view or all ([`write`]), pin a value ([`pins`]).
//! - **MCP** ([`tools`]): `storage_get` and `storage_set`; values only while a person allows it
//!   (`storage-panel.mcpValues`, protected).
//! - **Pins** ([`pins`], [`page`]): set again after every navigation - the storage values by the
//!   page script `page/pins.js` at document start, the cookies over CDP.
//!
//! Cookies come from CDP `Network` ([`cookie`]), the storage areas from `src/ops.js` run with
//! `evaluate` ([`read`]). Values are user data: never logged. The logic is plain JSON on the
//! native traits and tested natively; `guest.rs` translates it into the contract's vocabulary
//! and exists on `wasm32` alone.

#[cfg(target_arch = "wasm32")]
mod guest;

pub mod columns;
pub mod cookie;
pub mod matrix;
pub mod origin;
pub mod page;
pub mod panel;
pub mod pins;
pub mod read;
pub mod settings;
pub mod tools;
pub mod write;

// The fake browser of the tests names the crate as an outside test does.
#[cfg(test)]
extern crate self as storage_panel;

#[cfg(test)]
#[path = "../tests/support/browser.rs"]
mod browser;

use rb_plugin_api::feature::Feature;
use rb_plugin_api::{Failure, FailureKind, PluginHost, codes};
use serde_json::{Value, json};

/// The plugin's id: the prefix of its keys.
pub const PLUGIN_ID: &str = "storage-panel";
/// The panel the command `panel` shows.
pub const PANEL_ID: &str = "storage";
/// The answer key asking the host to show a panel of the plugin.
pub const OPEN_KEY: &str = "$open";
/// The command `panel`.
pub const PANEL: &str = "panel";
/// The tool `get` (`storage_get`).
pub const GET: &str = "get";
/// The tool `set` (`storage_set`).
pub const SET: &str = "set";
/// The page script's requests.
pub const PAGE_TOOL: &str = "@page";

/// The i18n key `plugin.storage-panel.<name>`.
#[must_use]
pub fn key(name: &str) -> String {
    format!("plugin.{PLUGIN_ID}.{name}")
}

pub(crate) fn coded(kind: FailureKind, name: &str) -> Failure {
    Failure::coded(kind, &key(name))
}

pub(crate) fn bad_args(reason: &str) -> Failure {
    let reason: String = reason.chars().take(64).collect();
    coded(FailureKind::BadData, "errors.bad_args").with_param("reason", reason)
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

fn answer(value: Result<Value, Failure>) -> Result<String, Failure> {
    value.map(|value| value.to_string())
}

/// The plugin.
pub struct StoragePanel;

impl Feature for StoragePanel {
    fn run_command(
        &mut self,
        host: &mut dyn PluginHost,
        command: &str,
        args: &str,
    ) -> Result<String, Failure> {
        let args = parse(args)?;
        match command {
            PANEL => answer(panel::command(host, &args)),
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
            GET => answer(tools::get(host, &args)),
            SET => answer(tools::set(host, &args)),
            PAGE_TOOL => answer(page::request(host, &args)),
            other => Err(unknown(other)),
        }
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
