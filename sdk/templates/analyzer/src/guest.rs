//! The component: `src/lib.rs` translated into the vocabulary of `wit/rbrowser-plugin.wit`. Built
//! only for `wasm32`; it holds no logic of its own, so it needs no test of its own.
#![allow(
    unsafe_code,
    reason = "the canonical-ABI glue wit-bindgen generates is the only unsafe code here"
)]

wit_bindgen::generate!({
    path: "wit",
    world: "analyzer-plugin",
});

use exports::rbrowser::plugin::analyzer::{Guest, Proposal};
use rbrowser::plugin::types::Failure;
use rbrowser::plugin::views;

struct Component;

impl Guest for Component {
    fn analyze(_args: String) -> Result<Vec<Proposal>, Failure> {
        let sizes: Vec<(u32, u32)> = views::list()
            .into_iter()
            .map(|view| (view.width, view.height))
            .collect();
        Ok(crate::propose(&sizes)
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

export!(Component);
