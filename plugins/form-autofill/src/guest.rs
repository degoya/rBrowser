//! The component: `src/lib.rs` translated into the vocabulary of the contract
//! (`crates/rb-plugin-api/wit`). Built only for `wasm32`; it holds no logic of its own.
//!
//! The component targets the world `feature-plugin`: the commands `fill_valid`, `fill_invalid`
//! and `capture` and the MCP tools `autofill_fill`, `autofill_capture`, `autofill_save`,
//! `autofill_presets`, `autofill_rename` and `autofill_delete`. The host it hands the native trait reaches the interfaces the manifest
//! grants (`settings`, `views`, `evaluate`, `project` and `out` - the presets' file, RB-090-70)
//! and refuses the rest here already, as the real host would, so the component imports nothing
//! else.
#![allow(
    unsafe_code,
    reason = "the canonical-ABI glue wit-bindgen generates is the only unsafe code here"
)]

wit_bindgen::generate!({
    inline: "
        package rbrowser:form-autofill;

        world plugin {
            include rbrowser:plugin/feature-plugin@0.1.0;
        }
    ",
    path: "../../crates/rb-plugin-api/wit",
    world: "rbrowser:form-autofill/plugin",
    // The interfaces come from another package (`rbrowser:plugin`): generate them here.
    generate_all,
});

use exports::rbrowser::plugin::feature::Guest as FeatureGuest;
use rb_plugin_api::feature::Feature;
use rbrowser::plugin::types;

use crate::FormAutofill;

rb_plugin_api::guest_host! {
    /// The host as the plugin sees it: the granted interfaces; everything else is refused here.
    struct GuestHost grants [settings, views, evaluate, project, out]
}

struct Component;

impl FeatureGuest for Component {
    fn activate() -> Result<(), types::Failure> {
        FormAutofill.activate(&mut GuestHost).map_err(to_wit)
    }

    fn run_command(command: String, args: String) -> Result<String, types::Failure> {
        FormAutofill
            .run_command(&mut GuestHost, &command, &args)
            .map_err(to_wit)
    }

    fn call_tool(tool: String, args: String) -> Result<String, types::Failure> {
        FormAutofill
            .call_tool(&mut GuestHost, &tool, &args)
            .map_err(to_wit)
    }
}

export!(Component);
