//! The component: `src/lib.rs` translated into the vocabulary of the contract
//! (`crates/rb-plugin-api/wit`). Built only for `wasm32`; it holds no logic of its own and imports
//! no interface - the plugin is granted no capability.
#![allow(
    unsafe_code,
    reason = "the canonical-ABI glue wit-bindgen generates is the only unsafe code here"
)]

wit_bindgen::generate!({
    path: "../../crates/rb-plugin-api/wit",
    world: "feature-plugin",
});

use exports::rbrowser::plugin::feature::Guest;
use rbrowser::plugin::types::{Failure, FailureKind};

struct Component;

fn failure(refusal: crate::Refusal) -> Failure {
    let kind = if refusal.code == "plugins.error.unknown_contribution" {
        FailureKind::NotFound
    } else {
        FailureKind::BadData
    };
    Failure {
        kind,
        code: Some(refusal.code.to_owned()),
        params: refusal.params,
    }
}

impl Guest for Component {
    fn activate() -> Result<(), Failure> {
        Ok(())
    }

    fn run_command(command: String, args: String) -> Result<String, Failure> {
        crate::run_command(&command, &args).map_err(failure)
    }

    fn call_tool(tool: String, args: String) -> Result<String, Failure> {
        crate::call_tool(&tool, &args).map_err(failure)
    }
}

export!(Component);
