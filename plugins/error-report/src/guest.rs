//! The component: `src/lib.rs` translated into the vocabulary of the contract
//! (`crates/rb-plugin-api/wit`). Built only for `wasm32`; it holds no logic of its own.
//!
//! The component targets a world of its own that includes `feature-plugin` and `check-plugin`
//! (`docs/plugins.md`, "Plugin kinds"). The host it hands the native traits reaches `host`,
//! `settings` and the five interfaces the manifest grants - `views`, `evaluate`, `project`, `out`
//! and `findings`; every other capability is refused here already, as the real host would refuse
//! it, and a component that calls no other interface imports none.
#![allow(
    unsafe_code,
    reason = "the canonical-ABI glue wit-bindgen generates is the only unsafe code here"
)]

wit_bindgen::generate!({
    inline: "
        package rbrowser:error-report;

        world plugin {
            include rbrowser:plugin/feature-plugin@0.1.0;
            include rbrowser:plugin/check-plugin@0.1.0;
        }
    ",
    path: "../../crates/rb-plugin-api/wit",
    world: "rbrowser:error-report/plugin",
    // The interfaces come from another package (`rbrowser:plugin`): generate them here.
    generate_all,
});

use exports::rbrowser::plugin::check::{
    CheckContext as WitContext, Guest as CheckGuest, RuleInfo as WitRule,
};
use exports::rbrowser::plugin::feature::Guest as FeatureGuest;
use rb_plugin_api::CheckContext;
use rb_plugin_api::check::Check;
use rb_plugin_api::feature::Feature;
use rbrowser::plugin::types;

use crate::ErrorReport;

rb_plugin_api::guest_host! {
    /// The host as the plugin sees it.
    struct GuestHost grants [settings, views, evaluate, project_read, out, findings]
}

struct Component;

impl FeatureGuest for Component {
    fn activate() -> Result<(), types::Failure> {
        ErrorReport.activate(&mut GuestHost).map_err(to_wit)
    }

    fn run_command(command: String, args: String) -> Result<String, types::Failure> {
        ErrorReport
            .run_command(&mut GuestHost, &command, &args)
            .map_err(to_wit)
    }

    fn call_tool(tool: String, args: String) -> Result<String, types::Failure> {
        ErrorReport
            .call_tool(&mut GuestHost, &tool, &args)
            .map_err(to_wit)
    }
}

impl CheckGuest for Component {
    fn describe() -> Vec<WitRule> {
        ErrorReport
            .describe()
            .into_iter()
            .map(|rule| WitRule {
                id: rule.id,
                title: rule.title,
                severity: severity_to_wit(rule.severity),
                shared_id: rule.shared_id,
                help_url: rule.help_url,
            })
            .collect()
    }

    fn run(ctx: WitContext) -> Result<Vec<types::Finding>, types::Failure> {
        let ctx = CheckContext {
            view: ctx.view,
            url: ctx.url,
            rules: ctx.rules,
            args: ctx.args,
        };
        let found = ErrorReport.run(&mut GuestHost, &ctx).map_err(to_wit)?;
        Ok(found.into_iter().map(finding_to_wit).collect())
    }
}

export!(Component);
