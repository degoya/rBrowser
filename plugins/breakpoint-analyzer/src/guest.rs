//! The component: `src/lib.rs` translated into the vocabulary of the contract
//! (`crates/rb-plugin-api/wit`). Built only for `wasm32`; it holds no logic of its own.
//!
//! The component targets a world of its own that includes `analyzer-plugin` and
//! `feature-plugin` (`docs/plugins.md`, "Plugin kinds"). The host it hands the native traits
//! reaches only the `host` interface: the plugin is granted no capability, and a component
//! that calls no other interface imports none.
#![allow(
    unsafe_code,
    reason = "the canonical-ABI glue wit-bindgen generates is the only unsafe code here"
)]

wit_bindgen::generate!({
    inline: "
        package rbrowser:breakpoint-analyzer;

        world plugin {
            include rbrowser:plugin/analyzer-plugin@0.1.0;
            include rbrowser:plugin/feature-plugin@0.1.0;
        }
    ",
    path: "../../crates/rb-plugin-api/wit",
    world: "rbrowser:breakpoint-analyzer/plugin",
    // The interfaces come from another package (`rbrowser:plugin`): generate them here.
    generate_all,
});

use exports::rbrowser::plugin::analyzer::{Guest as AnalyzerGuest, Proposal};
use exports::rbrowser::plugin::feature::Guest as FeatureGuest;
use rb_plugin_api::analyzer::Analyzer;
use rb_plugin_api::feature::Feature;
use rbrowser::plugin::types;

use crate::BreakpointAnalyzer;

rb_plugin_api::guest_host! {
    /// The host as the plugin sees it: logging and the clock; every capability is refused here
    /// already, as the real host would refuse it.
    struct GuestHost grants []
}

struct Component;

impl AnalyzerGuest for Component {
    fn analyze(args: String) -> Result<Vec<Proposal>, types::Failure> {
        let proposals = BreakpointAnalyzer
            .analyze(&mut GuestHost, &args)
            .map_err(to_wit)?;
        Ok(proposals
            .into_iter()
            .map(|proposal| Proposal {
                pointer: proposal.pointer,
                value: proposal.value,
                reason: proposal.reason,
                params: proposal.params,
            })
            .collect())
    }
}

impl FeatureGuest for Component {
    fn activate() -> Result<(), types::Failure> {
        BreakpointAnalyzer.activate(&mut GuestHost).map_err(to_wit)
    }

    fn run_command(command: String, args: String) -> Result<String, types::Failure> {
        BreakpointAnalyzer
            .run_command(&mut GuestHost, &command, &args)
            .map_err(to_wit)
    }

    fn call_tool(tool: String, args: String) -> Result<String, types::Failure> {
        BreakpointAnalyzer
            .call_tool(&mut GuestHost, &tool, &args)
            .map_err(to_wit)
    }
}

export!(Component);
