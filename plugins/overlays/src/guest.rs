//! The component: `src/lib.rs` translated into the vocabulary of the contract
//! (`crates/rb-plugin-api/wit`). Built only for `wasm32`; it holds no logic of its own.
//!
//! The host it hands the native trait reaches `host`, `settings` and the six interfaces the
//! manifest grants - `views`, `evaluate`, `cdp`, `findings`, `project` (`project-read`) and
//! `out` (`out-write`); a component that calls no other interface imports none, so the rest
//! stay refused here.
#![allow(
    unsafe_code,
    reason = "the canonical-ABI glue wit-bindgen generates is the only unsafe code here"
)]

wit_bindgen::generate!({
    path: "../../crates/rb-plugin-api/wit",
    world: "feature-plugin",
});

use exports::rbrowser::plugin::feature::Guest;
use rb_plugin_api::feature::Feature;
use rbrowser::plugin::types;

use crate::Overlays;

rb_plugin_api::guest_host! {
    /// The host as the plugin sees it.
    struct GuestHost grants [settings, views, evaluate, cdp, project, out, findings]
}

struct Component;

impl Guest for Component {
    fn activate() -> Result<(), types::Failure> {
        Overlays.activate(&mut GuestHost).map_err(to_wit)
    }

    fn run_command(command: String, args: String) -> Result<String, types::Failure> {
        Overlays
            .run_command(&mut GuestHost, &command, &args)
            .map_err(to_wit)
    }

    fn call_tool(tool: String, args: String) -> Result<String, types::Failure> {
        Overlays
            .call_tool(&mut GuestHost, &tool, &args)
            .map_err(to_wit)
    }
}

export!(Component);
