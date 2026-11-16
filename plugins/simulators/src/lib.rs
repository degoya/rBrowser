//! Simulators (RB-070-08): colour vision deficiencies, vision impairments and surroundings,
//! chosen per view - one view as it is, the next as a person with deuteranopia sees it.
//!
//! **Where the work happens.** This component holds the catalogue ([`catalog`]: the fifteen
//! simulations, their matrices and strengths), each view's choice ([`state`], kept in
//! `.rbrowser/out/simulators/` because a component lives for one call) and the settings. Chromium
//! renders six types itself (CDP `Emulation.setEmulatedVisionDeficiency`, [`apply`]); every other
//! type, every strength other than Chromium's and every Firefox or WebKit view is drawn by the
//! page script (`page/simulators.js`): a CSS `filter` on the root element - the one element
//! whose filter makes no containing block for `position: fixed` - and, for the ones that hide
//! part of the view, a layer that takes no pointer events, so sync and clicks work as before.
//!
//! Contributions (`manifest.toml`):
//!
//! - the command `pick` (`Ctrl+Shift+Space O I`) opens the picker, the dialog panel `picker`;
//! - the MCP tool `simulator_set` ([`set`]), covering the command; the picker calls it too;
//! - the page script's requests ([`page`]).
//!
//! The logic is tested natively against `rb_plugin_api::test_support::MemoryHost`; `guest.rs`
//! translates it into the WIT vocabulary and exists on `wasm32` alone.

#[cfg(target_arch = "wasm32")]
mod guest;

pub mod apply;
pub mod catalog;
pub mod page;
pub mod set;
pub mod state;

#[cfg(test)]
mod testing;

use rb_plugin_api::feature::Feature;
use rb_plugin_api::{Failure, FailureKind, PluginHost};
use serde_json::json;

/// The plugin's id, as in `manifest.toml`.
pub const PLUGIN_ID: &str = "simulators";
/// The tool a page script's request reaches `call-tool` with (`rb_plugin_host::PAGE_MESSAGE_TOOL`).
pub const PAGE_TOOL: &str = "@page";
/// The MCP tool `simulator_set`.
pub const SET_TOOL: &str = "set";
/// The picker's panel id.
pub const PICKER_PANEL: &str = "picker";

/// `plugin.simulators.errors.bad_args` naming what does not fit.
#[must_use]
pub fn bad_args(reason: &str) -> Failure {
    Failure::coded(
        FailureKind::BadData,
        &format!("plugin.{PLUGIN_ID}.errors.bad_args"),
    )
    .with_param("reason", reason.chars().take(64).collect::<String>())
}

fn unknown(name: &str) -> Failure {
    Failure::coded(FailureKind::NotFound, "plugins.error.unknown_contribution")
        .with_param("name", name.chars().take(64).collect::<String>())
}

/// The plugin's `feature` export.
#[derive(Clone, Copy, Debug, Default)]
pub struct Simulators;

impl Feature for Simulators {
    /// A start without `persist` forgets the last session's choices.
    fn activate(&mut self, host: &mut dyn PluginHost) -> Result<(), Failure> {
        if !state::Settings::read(host).persist {
            state::clear(host);
        }
        Ok(())
    }

    fn run_command(
        &mut self,
        _host: &mut dyn PluginHost,
        command: &str,
        _args: &str,
    ) -> Result<String, Failure> {
        match command {
            "pick" => Ok(json!({ "$open": { "panel": PICKER_PANEL } }).to_string()),
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
            SET_TOOL => set::run(host, args),
            PAGE_TOOL => page::handle(host, args),
            other => Err(unknown(other)),
        }
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
