//! The component: `src/lib.rs` translated into the vocabulary of the contract
//! (`crates/rb-plugin-api/wit`). Built only for `wasm32`; it holds no logic of its own.
//!
//! The component targets `feature-plugin`. The host it hands the native traits reaches `host`,
//! `settings` and the two interfaces the manifest grants - `project` and `project-write`; every
//! other capability is refused here already, as the real host would refuse it, and a component
//! that calls no other interface imports none.
#![allow(
    unsafe_code,
    reason = "the canonical-ABI glue wit-bindgen generates is the only unsafe code here"
)]

wit_bindgen::generate!({
    inline: "
        package rbrowser:todo;

        world plugin {
            include rbrowser:plugin/feature-plugin@0.1.0;
        }
    ",
    path: "../../crates/rb-plugin-api/wit",
    world: "rbrowser:todo/plugin",
    // The interfaces come from another package (`rbrowser:plugin`): generate them here.
    generate_all,
});

use exports::rbrowser::plugin::feature::Guest as FeatureGuest;
use rb_plugin_api::feature::Feature;
use rbrowser::plugin::types;

use crate::Todo;

rb_plugin_api::guest_host! {
    /// The host as the plugin sees it.
    struct GuestHost grants [settings, project, project_write]
}

struct Component;

impl FeatureGuest for Component {
    fn activate() -> Result<(), types::Failure> {
        Todo.activate(&mut GuestHost).map_err(to_wit)
    }

    fn run_command(command: String, args: String) -> Result<String, types::Failure> {
        Todo.run_command(&mut GuestHost, &command, &args)
            .map_err(to_wit)
    }

    fn call_tool(tool: String, args: String) -> Result<String, types::Failure> {
        Todo.call_tool(&mut GuestHost, &tool, &args).map_err(to_wit)
    }
}

export!(Component);
