//! Live CSS (RB-094-04, `docs/plugins.md`): a CSS draft that applies at once in every view.
//!
//! - **The draft** ([`draft`]): rule blocks, each for every view or for chosen breakpoints, and
//!   whether the views apply it - off keeps the blocks.
//! - **The scope** ([`media`]): a block for chosen breakpoints applies inside the media query of
//!   their widths, as the grid's breakpoints set them; what the views apply is what a person
//!   copies.
//! - **Kept** ([`store`]) in `.rbrowser/out/live-css/` (`draft.json`, and the stylesheet
//!   `draft.css`), read and written only in a trusted workspace.
//! - **Applied** by the page script (`page/live-css.js`) in the plugin's isolated world: one
//!   constructed stylesheet per view in `document.adoptedStyleSheets`, at the start of every
//!   document (setting `applyOnNavigate`) and at every change the component posts ([`tools`]).
//!
//! Contributions (`manifest.toml`): the tools `set` and `clear` (MCP `livecss_set`,
//! `livecss_clear`), the commands `panel` (`Ctrl+Shift+Space P Q`) and `toggle`
//! (`Ctrl+Shift+Space D L`), the panel `editor`, the page script's requests.
//!
//! The logic is plain JSON on the native traits and tested natively; `guest.rs` translates it
//! into the contract's vocabulary and exists on `wasm32` alone.

#[cfg(target_arch = "wasm32")]
mod guest;

pub mod draft;
pub mod media;
pub mod store;
pub mod tools;

#[cfg(test)]
mod testing;

use rb_plugin_api::feature::Feature;
use rb_plugin_api::{Failure, FailureKind, PluginHost, codes};
use serde_json::{Value, json};

/// The plugin's id: the prefix of its keys.
pub const PLUGIN_ID: &str = "live-css";
/// The tool a page script's request reaches `call-tool` with (`rb_plugin_host::PAGE_MESSAGE_TOOL`).
pub const PAGE_TOOL: &str = "@page";
/// The tools.
pub const SET_TOOL: &str = "set";
pub const CLEAR_TOOL: &str = "clear";
/// The commands.
pub const PANEL_COMMAND: &str = "panel";
pub const TOGGLE_COMMAND: &str = "toggle";
/// The panel the command `panel` shows.
pub const EDITOR_PANEL: &str = "editor";

/// The i18n key `plugin.live-css.<name>`.
#[must_use]
pub fn key(name: &str) -> String {
    format!("plugin.{PLUGIN_ID}.{name}")
}

pub(crate) fn coded(kind: FailureKind, name: &str) -> Failure {
    Failure::coded(kind, &key(name))
}

/// `plugin.live-css.errors.bad_args` naming what does not read.
pub(crate) fn bad_args(reason: &str) -> Failure {
    coded(FailureKind::BadData, "errors.bad_args")
        .with_param("reason", reason.chars().take(64).collect::<String>())
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

fn answer(value: Result<Value, Failure>) -> Result<String, Failure> {
    value.map(|value| value.to_string())
}

/// The plugin's `feature` export.
#[derive(Clone, Copy, Debug, Default)]
pub struct LiveCss;

impl Feature for LiveCss {
    fn run_command(
        &mut self,
        host: &mut dyn PluginHost,
        command: &str,
        args: &str,
    ) -> Result<String, Failure> {
        parse(args)?;
        match command {
            PANEL_COMMAND => answer(Ok(json!({ "$open": { "panel": EDITOR_PANEL } }))),
            TOGGLE_COMMAND => answer(tools::toggle(host)),
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
            SET_TOOL => answer(tools::set(host, &args)),
            CLEAR_TOOL => answer(tools::clear(host)),
            PAGE_TOOL => answer(tools::page(host, &args)),
            other => Err(unknown(other)),
        }
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
