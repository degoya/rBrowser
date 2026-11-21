//! Container queries made visible (RB-094-01, `docs/plugins.md`): every element with a size
//! `container-type` framed in every view - its name, its inline and block size and the
//! `@container` conditions that match now -, the analysis of all `@container` conditions of the
//! page with their bounds and, per container, the next bound above and below and the breakpoints
//! that switch it, and one container's width dragged inline to test its rules without changing
//! the view.
//!
//! Three parts:
//!
//! - the **page script** `page/containers.js` (built from `web/src/`) draws the overlay in a
//!   closed shadow root of a layer in the page's `<rbrowser-layers>` and lets a handle drag a
//!   container's width;
//! - the **component** (this crate) keeps whether the overlay shows per project ([`state`]),
//!   answers the commands and MCP tools ([`tools`]), evaluates the analysis in the views
//!   ([`script`]) and merges it ([`analysis`]);
//! - the **panel** `panel/index.html` (built from `web/src/panel/`) lists the conditions and the
//!   containers per view and resets a dragged width.
//!
//! The MCP tools are `containers_overlay` (`overlay`), `containers_analyze` (`analyze`) and
//! `containers_resize` (`resize`); the command `overlay` is covered by the first, `analyze` by the
//! second.
//!
//! The logic is tested natively against `rb_plugin_api::test_support::MemoryHost`; `guest.rs`
//! translates it into the WIT vocabulary and exists on `wasm32` alone.

pub mod analysis;
#[cfg(test)]
mod fake;
#[cfg(target_arch = "wasm32")]
mod guest;
pub mod script;
pub mod settings;
pub mod state;
pub mod tools;

use rb_plugin_api::feature::Feature;
use rb_plugin_api::{Failure, FailureKind, PluginHost, codes};

/// The plugin's id: the prefix of its keys and its out folder.
pub const PLUGIN_ID: &str = "container-queries";
/// The tool a page script's request reaches `call-tool` with (`rb_plugin_host::PAGE_MESSAGE_TOOL`).
pub const PAGE_TOOL: &str = "@page";

/// The plugin.
#[derive(Clone, Copy, Debug, Default)]
pub struct ContainerQueries;

fn unknown(name: &str) -> Failure {
    Failure::coded(FailureKind::NotFound, codes::UNKNOWN_CONTRIBUTION)
        .with_param("name", name.chars().take(64).collect::<String>())
}

impl Feature for ContainerQueries {
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
        match command {
            "overlay" => tools::overlay(host, args),
            "analyze" => Ok(tools::analyze_command()),
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
            "overlay" => tools::overlay(host, args),
            "analyze" => tools::analyze(host, args),
            "resize" => tools::resize(host, args),
            PAGE_TOOL => tools::page(host, args),
            other => Err(unknown(other)),
        }
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
