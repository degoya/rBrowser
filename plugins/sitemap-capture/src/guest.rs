//! The component: `src/lib.rs` translated into the vocabulary of the contract
//! (`crates/rb-plugin-api/wit`), world `feature-plugin`. Built only for `wasm32`; it holds no
//! logic of its own. The host it hands the native trait reaches `host`, `settings` (the manifest
//! declares a schema) and `http` - the one capability the manifest grants; every other one is
//! refused here already, as the real host would refuse it, and a component that calls no other
//! interface imports none.
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

use crate::SitemapCapture;

rb_plugin_api::guest_host! {
    /// The host as the plugin sees it: logging, the clock, its own settings and `http.fetch`.
    struct GuestHost grants [settings, http]
}

struct Component;

impl Guest for Component {
    fn activate() -> Result<(), types::Failure> {
        Ok(())
    }

    fn run_command(command: String, args: String) -> Result<String, types::Failure> {
        SitemapCapture
            .run_command(&mut GuestHost, &command, &args)
            .map_err(to_wit)
    }

    fn call_tool(tool: String, args: String) -> Result<String, types::Failure> {
        SitemapCapture
            .call_tool(&mut GuestHost, &tool, &args)
            .map_err(to_wit)
    }
}

export!(Component);
