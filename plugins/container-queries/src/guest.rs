//! The component: `src/lib.rs` translated into the vocabulary of the contract
//! (`crates/rb-plugin-api/wit`). Built only for `wasm32`; it holds no logic of its own.
//!
//! The host it hands the native trait reaches `host`, `settings` and the four interfaces the
//! manifest grants - `views`, `evaluate`, `project` (`project-read`) and `out` (`out-write`); a
//! component that calls no other interface imports none, so the rest stay refused here.
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

use crate::ContainerQueries;

rb_plugin_api::guest_host! {
    /// The host as the plugin sees it.
    struct GuestHost grants [settings, views, evaluate, project, out]
}

struct Component;

impl Guest for Component {
    fn activate() -> Result<(), types::Failure> {
        ContainerQueries.activate(&mut GuestHost).map_err(to_wit)
    }

    fn run_command(command: String, args: String) -> Result<String, types::Failure> {
        ContainerQueries
            .run_command(&mut GuestHost, &command, &args)
            .map_err(to_wit)
    }

    fn call_tool(tool: String, args: String) -> Result<String, types::Failure> {
        ContainerQueries
            .call_tool(&mut GuestHost, &tool, &args)
            .map_err(to_wit)
    }
}

export!(Component);
