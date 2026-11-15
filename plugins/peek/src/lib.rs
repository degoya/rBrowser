//! Peek (RB-060-04, `docs/plugins.md`): the Alt+hover inspector. Holding the modifier
//! (`peek.modifier`, Alt by default) over an element shows a tooltip with its tag, id, classes,
//! size, colours and WCAG contrast, font, box model and accessibility role and name, and outlines
//! the same element in the other views (`peek.syncHighlight`).
//!
//! - The **page script** (`page/peek.js`, built from `web/`) watches the modifier and the mouse,
//!   reads the element's computed style, draws box model and tooltip in a closed shadow root and
//!   asks the component (`@page`, [`page`]).
//! - The **component** checks the page's data, rates the contrast over the background resolved
//!   through the ancestors ([`contrast`], [`background`]), adds what CDP knows in Chromium -
//!   rendered fonts, role and name ([`node`]) - and posts the element to the other views.
//! - The command `toggle` (`Ctrl+Shift+Space D P`) switches the inspector in every view; the
//!   MCP tool `peek_inspect` ([`tool`]) answers the tooltip's data as JSON.
//!
//! The logic is plain JSON against the native [`PluginHost`]; `guest.rs` translates it into the
//! contract's vocabulary and exists on `wasm32` alone.

#[cfg(target_arch = "wasm32")]
mod guest;

pub mod background;
pub mod color;
pub mod config;
pub mod contrast;
pub mod inspect;
pub mod node;
pub mod page;
pub mod tool;

#[cfg(test)]
mod testing;

use rb_plugin_api::feature::Feature;
use rb_plugin_api::{Failure, FailureKind, PluginHost, codes};
use serde_json::json;

/// The plugin's id: the prefix of its keys.
pub const PLUGIN_ID: &str = "peek";
/// The tool a page script's request reaches `call-tool` with (`rb_plugin_host::PAGE_MESSAGE_TOOL`).
pub const PAGE_TOOL: &str = "@page";

fn unknown(name: &str) -> Failure {
    Failure::coded(FailureKind::NotFound, codes::UNKNOWN_CONTRIBUTION)
        .with_param("name", name.chars().take(64).collect::<String>())
}

/// `toggle`: every view's page script switches the inspector on or off for its document; a new
/// document starts from `peek.enabled` again.
#[must_use]
pub fn toggle() -> String {
    json!({ "$post": [{ "target": "all", "message": { "type": "toggle" } }] }).to_string()
}

/// The feature plugin.
#[derive(Clone, Copy, Debug, Default)]
pub struct Peek;

impl Feature for Peek {
    fn run_command(
        &mut self,
        _host: &mut dyn PluginHost,
        command: &str,
        _args: &str,
    ) -> Result<String, Failure> {
        match command {
            "toggle" => Ok(toggle()),
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
            "inspect" => tool::inspect(host, args),
            PAGE_TOOL => page::handle(host, args),
            other => Err(unknown(other)),
        }
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
