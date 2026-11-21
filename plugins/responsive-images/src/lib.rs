//! rBrowser's first-party responsive images check (RB-093-02, `docs/plugins.md`): in every view,
//! which file the browser took from `srcset` or `<picture>`, and whether it is too large or too
//! small for the size it is drawn at times the view's pixel density; a `srcset` with widths and no
//! `sizes`; an LCP image that loads lazily or without `fetchpriority="high"`. Live, per view and
//! its density, without a Lighthouse run.
//!
//! The collector (`web/src/collect.ts`, built into `src/collect.js`) reads and measures each
//! view's images through `evaluate` in the plugin's isolated world; the mark of an image (the
//! panel's hover) comes over CDP (`DOM`, `Overlay`). A `check-plugin` for `checks_run` and the
//! findings panel, a `feature-plugin` for the MCP tools `images_check` and `images_mark`, the panel
//! and its commands. The logic is native Rust over [`PluginHost`], tested against fakes;
//! `src/guest.rs` translates it for the component.

use rb_plugin_api::check::Check;
use rb_plugin_api::feature::Feature;
use rb_plugin_api::{CheckContext, Failure, FailureKind, Finding, PluginHost, RuleInfo, codes};
use serde_json::{Value, json};

pub mod levels;
pub mod mark;
pub mod page;
pub mod rules;
pub mod run;
pub mod settings;
pub mod tool;

#[cfg(test)]
mod fake;

#[cfg(target_arch = "wasm32")]
mod guest;

/// The plugin's id.
pub const PLUGIN_ID: &str = "responsive-images";
/// The panel the command `panel` shows.
pub const PANEL_ID: &str = "responsive-images";
/// The answer key asking the host to show a panel of the plugin.
pub const OPEN_KEY: &str = "$open";
/// The answer key carrying posts to the plugin's panels.
pub const POST_KEY: &str = "$post";

/// The i18n key `plugin.responsive-images.<name>`.
#[must_use]
pub fn key(name: &str) -> String {
    format!("plugin.{PLUGIN_ID}.{name}")
}

/// The plugin: no state between calls (a store lives for one call).
#[derive(Clone, Copy, Debug, Default)]
pub struct ResponsiveImages;

impl Check for ResponsiveImages {
    fn describe(&self) -> Vec<RuleInfo> {
        rules::describe()
    }

    fn run(
        &mut self,
        host: &mut dyn PluginHost,
        ctx: &CheckContext,
    ) -> Result<Vec<Finding>, Failure> {
        run::check(host, ctx)
    }
}

fn answer(value: Result<Value, Failure>) -> Result<String, Failure> {
    value.map(|value| value.to_string())
}

impl Feature for ResponsiveImages {
    fn run_command(
        &mut self,
        host: &mut dyn PluginHost,
        command: &str,
        args: &str,
    ) -> Result<String, Failure> {
        let args = tool::parse(args)?;
        match command {
            "panel" => answer(Ok(json!({ OPEN_KEY: { "panel": PANEL_ID } }))),
            "check" => answer(tool::check_command(host, &args)),
            _ => Err(unknown(command)),
        }
    }

    fn call_tool(
        &mut self,
        host: &mut dyn PluginHost,
        tool: &str,
        args: &str,
    ) -> Result<String, Failure> {
        let args = tool::parse(args)?;
        match tool {
            "check" => answer(tool::check(host, &args)),
            "mark" => answer(tool::mark(host, &args)),
            _ => Err(unknown(tool)),
        }
    }
}

fn unknown(name: &str) -> Failure {
    Failure::coded(FailureKind::NotFound, codes::UNKNOWN_CONTRIBUTION).with_param("name", name)
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
