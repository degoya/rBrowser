//! The debug overlays (RB-060-03, RB-070-07, `docs/plugins.md`): the overflow detector, the
//! element highlight (a CSS selector searched in every view), the layout outlines, and the
//! further overlays - stacking contexts, unneeded scrollbars, WCAG text spacing, target size,
//! small text, focus tracking, forced states, content chaos, the placeholdifier, and CSS,
//! JavaScript and images off; and the tab order (RB-070-12, [`tab_order`], [`tab_compare`]).
//!
//! Three parts:
//!
//! - the **page script** `page/overlays.js` (built from `web/src/`) draws every overlay in a
//!   closed shadow root of the plugin's isolated world, measures overflow and asks the component
//!   for the state when a document starts;
//! - the **component** (this crate) keeps the state per project ([`state`]), answers the commands
//!   and MCP tools ([`tools`]), scans the views and forces states over CDP ([`effects`],
//!   [`scan`]) and posts every change to the page scripts and the panel;
//! - the **panel** `panel/index.html` (built from `web/src/panel/`) switches the overlays and
//!   takes the selector, with the arrow keys stepping through the matches.
//!
//! The MCP tools are `overlay_set` (`set`), `overlay_overflow` (`overflow`), `overlay_state`
//! (`state`) and `overlay_tab_order` (`tab_order`); every command is covered by `overlay_set`. The overlays are a registry
//! ([`overlay::OVERLAYS`], [`overlay::COMMANDS`]), so a further overlay joins without a new tool.
//!
//! The logic is tested natively against `rb_plugin_api::test_support::MemoryHost`; `guest.rs`
//! translates it into the WIT vocabulary and exists on `wasm32` alone.

pub mod budget;
pub mod effects;
#[cfg(target_arch = "wasm32")]
mod guest;
pub mod overlay;
pub mod report;
pub mod scan;
pub mod settings;
pub mod state;
pub mod tab_compare;
pub mod tab_order;
pub mod tools;

use rb_plugin_api::feature::Feature;
use rb_plugin_api::{Failure, FailureKind, PluginHost, codes};

/// The plugin's id: the prefix of its keys and its out folder.
pub const PLUGIN_ID: &str = "overlays";
/// The tool a page script's request reaches `call-tool` with (`rb_plugin_host::PAGE_MESSAGE_TOOL`).
pub const PAGE_TOOL: &str = "@page";

/// The plugin.
#[derive(Clone, Copy, Debug, Default)]
pub struct Overlays;

fn unknown(name: &str) -> Failure {
    Failure::coded(FailureKind::NotFound, codes::UNKNOWN_CONTRIBUTION)
        .with_param("name", name.chars().take(64).collect::<String>())
}

impl Feature for Overlays {
    fn activate(&mut self, host: &mut dyn PluginHost) -> Result<(), Failure> {
        tools::activate(host);
        Ok(())
    }

    fn run_command(
        &mut self,
        host: &mut dyn PluginHost,
        command: &str,
        args: &str,
    ) -> Result<String, Failure> {
        if command == "off" || overlay::command_overlay(command).is_some() {
            tools::command(host, command, args)
        } else {
            Err(unknown(command))
        }
    }

    fn call_tool(
        &mut self,
        host: &mut dyn PluginHost,
        tool: &str,
        args: &str,
    ) -> Result<String, Failure> {
        match tool {
            "set" => tools::set(host, args),
            "overflow" => tools::overflow(host, args),
            "state" => tools::state(host, args),
            "tab_order" => tab_order::tool(host, args),
            PAGE_TOOL => tools::page(host, args),
            other => Err(unknown(other)),
        }
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
