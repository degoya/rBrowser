//! The component: `src/lib.rs` translated into the vocabulary of the contract
//! (`crates/rb-plugin-api/wit`). Built only for `wasm32`; it holds no logic of its own.
//!
//! The component targets a world of its own that includes `check-plugin` and `feature-plugin`
//! (`docs/plugins.md`, "Plugin kinds"): the check for `checks_run` and the findings panel, the
//! feature for the MCP tools `headers_check` and `headers_compare`, the panel and the commands.
//! The host it hands the native traits reaches the interfaces the manifest grants (`settings`,
//! `views`, `cdp`, `http`, `project` and `findings`) and refuses the rest here already, as the
//! real host would.
#![allow(
    unsafe_code,
    reason = "the canonical-ABI glue wit-bindgen generates is the only unsafe code here"
)]

wit_bindgen::generate!({
    inline: "
        package rbrowser:security-headers;

        world plugin {
            include rbrowser:plugin/check-plugin@0.1.0;
            include rbrowser:plugin/feature-plugin@0.1.0;
        }
    ",
    path: "../../crates/rb-plugin-api/wit",
    world: "rbrowser:security-headers/plugin",
    // The interfaces come from another package (`rbrowser:plugin`): generate them here.
    generate_all,
});

use exports::rbrowser::plugin::check::{CheckContext, Guest as CheckGuest, RuleInfo};
use exports::rbrowser::plugin::feature::Guest as FeatureGuest;
use rb_plugin_api::check::Check;
use rb_plugin_api::feature::Feature;
use rbrowser::plugin::types;

use crate::SecurityHeaders;

rb_plugin_api::guest_host! {
    /// The host as the plugin sees it: the granted interfaces; `evaluate`, `out` and `sidecar` are
    /// not granted and refused here, so the component does not import them.
    struct GuestHost grants [settings, views, cdp, http, project, findings]
}

struct Component;

impl CheckGuest for Component {
    fn describe() -> Vec<RuleInfo> {
        SecurityHeaders
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
        let findings = SecurityHeaders.run(&mut GuestHost, &ctx).map_err(to_wit)?;
        Ok(findings.into_iter().map(finding_to_wit).collect())
    }
}

impl FeatureGuest for Component {
    fn activate() -> Result<(), types::Failure> {
        SecurityHeaders.activate(&mut GuestHost).map_err(to_wit)
    }

    fn run_command(command: String, args: String) -> Result<String, types::Failure> {
        SecurityHeaders
            .run_command(&mut GuestHost, &command, &args)
            .map_err(to_wit)
    }

    fn call_tool(tool: String, args: String) -> Result<String, types::Failure> {
        SecurityHeaders
            .call_tool(&mut GuestHost, &tool, &args)
            .map_err(to_wit)
    }
}

export!(Component);
