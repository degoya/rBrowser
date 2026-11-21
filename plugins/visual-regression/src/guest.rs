//! The component: `src/lib.rs` translated into the vocabulary of the contract
//! (`crates/rb-plugin-api/wit`), world `check-plugin`. Built only for `wasm32`; it holds no logic
//! of its own. The host it hands the native trait reaches only the `host` interface: the plugin
//! is granted no capability, and a component that calls no other interface imports none.
#![allow(
    unsafe_code,
    reason = "the canonical-ABI glue wit-bindgen generates is the only unsafe code here"
)]

wit_bindgen::generate!({
    path: "../../crates/rb-plugin-api/wit",
    world: "check-plugin",
});

use exports::rbrowser::plugin::check::{CheckContext, Guest, RuleInfo};
use rb_plugin_api::check::Check;
use rbrowser::plugin::types;

use crate::VisualRegression;

rb_plugin_api::guest_host! {
    /// The host as the plugin sees it: logging and the clock; every capability is refused here
    /// already, as the real host would refuse it.
    struct GuestHost grants []
}

struct Component;

impl Guest for Component {
    fn describe() -> Vec<RuleInfo> {
        VisualRegression
            .describe()
            .into_iter()
            .map(|rule| RuleInfo {
                id: rule.id,
                title: rule.title,
                severity: severity_to_wit(rule.severity),
                shared_id: rule.shared_id,
                help_url: rule.help_url,
            })
            .collect()
    }

    fn run(ctx: CheckContext) -> Result<Vec<types::Finding>, types::Failure> {
        let ctx = rb_plugin_api::CheckContext {
            view: ctx.view,
            url: ctx.url,
            rules: ctx.rules,
            args: ctx.args,
        };
        let findings = VisualRegression.run(&mut GuestHost, &ctx).map_err(to_wit)?;
        Ok(findings.into_iter().map(finding_to_wit).collect())
    }
}

export!(Component);
