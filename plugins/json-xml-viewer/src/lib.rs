//! JSON/XML viewer (RB-070-05, `docs/plugins.md`): a JSON or XML document opened in a view -
//! an API response, a `sitemap.xml`, a feed - is shown as a collapsible, searchable tree with
//! a switch to its raw and formatted text, instead of the engine's plain text.
//!
//! - The **page script** (`page/viewer.js`, built from `web/`) does the work in the view's
//!   isolated world: it detects the document by content type and shape, parses it, renders the
//!   tree virtually in a closed shadow root, filters by text, JSONPath or XPath and copies values
//!   and paths. Large documents never cross the bridge.
//! - The **component** hands the page script its settings (`@page` `config`, [`config`]) and
//!   serves the command `toggle_raw` (`Ctrl+Shift+Space V J`), which switches the active view -
//!   or every view, with `{"target": "all"}` - between tree and raw text.
//!
//! No MCP tool: the viewer is presentation only; agents read responses through
//! `network_requests`. The logic is plain JSON against the native [`PluginHost`]; `guest.rs`
//! translates it into the contract's vocabulary and exists on `wasm32` alone.

#[cfg(target_arch = "wasm32")]
mod guest;

pub mod config;

use rb_plugin_api::feature::Feature;
use rb_plugin_api::{Failure, FailureKind, PluginHost, ViewId, codes};
use serde_json::{Value, json};

use crate::config::Config;

/// The plugin's id: the prefix of its keys.
pub const PLUGIN_ID: &str = "json-xml-viewer";
/// The tool a page script's request reaches `call-tool` with (`rb_plugin_host::PAGE_MESSAGE_TOOL`).
pub const PAGE_TOOL: &str = "@page";
/// The command switching between tree and raw text.
pub const TOGGLE_RAW: &str = "toggle_raw";

fn unknown(name: &str) -> Failure {
    Failure::coded(FailureKind::NotFound, codes::UNKNOWN_CONTRIBUTION)
        .with_param("name", name.chars().take(64).collect::<String>())
}

fn invalid(reason: &str) -> Failure {
    Failure::coded(FailureKind::BadData, codes::INVALID).with_param("reason", reason)
}

/// `args` as a JSON object; empty text is `{}` (a shortcut's call).
fn object(args: &str) -> Result<Value, Failure> {
    if args.trim().is_empty() {
        return Ok(json!({}));
    }
    let value: Value = serde_json::from_str(args).map_err(|_| invalid("args"))?;
    if value.is_object() {
        Ok(value)
    } else {
        Err(invalid("args"))
    }
}

/// `toggle_raw`: posts `toggleRaw` to the view `target` names - `"all"`, a view id - or, without
/// one, to the active view (every view when there is none or the host does not say).
pub fn toggle_raw(host: &mut dyn PluginHost, args: &str) -> Result<String, Failure> {
    let args = object(args)?;
    let target = match &args["target"] {
        Value::Null => host
            .active_view()
            .ok()
            .flatten()
            .map_or_else(|| json!("all"), |view| json!(view)),
        Value::String(text) if text == "all" => json!("all"),
        Value::Number(number) => {
            let view = number
                .as_u64()
                .and_then(|id| ViewId::try_from(id).ok())
                .ok_or_else(|| invalid("target"))?;
            json!(view)
        }
        _ => return Err(invalid("target")),
    };
    Ok(json!({ "$post": [{ "target": target, "message": { "type": "toggleRaw" } }] }).to_string())
}

/// A page script's request: `{"type": "config"}` answers the settings.
pub fn page(host: &mut dyn PluginHost, args: &str) -> Result<String, Failure> {
    let args = object(args)?;
    match args["message"]["type"].as_str() {
        Some("config") => Ok(json!({ "config": Config::read(host).to_json() }).to_string()),
        _ => Err(invalid("message")),
    }
}

/// The feature plugin.
#[derive(Clone, Copy, Debug, Default)]
pub struct Viewer;

impl Feature for Viewer {
    fn run_command(
        &mut self,
        host: &mut dyn PluginHost,
        command: &str,
        args: &str,
    ) -> Result<String, Failure> {
        match command {
            TOGGLE_RAW => toggle_raw(host, args),
            other => Err(unknown(other)),
        }
    }

    fn call_tool(
        &mut self,
        host: &mut dyn PluginHost,
        tool: &str,
        args: &str,
    ) -> Result<String, Failure> {
        match tool {
            PAGE_TOOL => page(host, args),
            other => Err(unknown(other)),
        }
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
