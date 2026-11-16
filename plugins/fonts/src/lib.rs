//! rBrowser's first-party font check (RB-070-13, `docs/plugins.md`): the fonts of every view -
//! families, faces, sources, formats, file sizes, where each is used - what Chromium really drew
//! against what the page declares, the font sizes across the breakpoints, and findings on
//! privacy (Google and Adobe Fonts), performance and readability for `checks_run`.
//!
//! The page part (`web/src`, built into `src/page.js`) reads each view's document through
//! `evaluate` in the plugin's isolated world and draws the marks; the rendered fonts come over CDP
//! (`Accessibility`, `DOM`, `CSS`, without dropping other clients' node ids); the export goes to
//! `.rbrowser/out/fonts/`. The logic is native Rust over [`PluginHost`], tested against fakes;
//! `src/guest.rs` translates it for the component.

use rb_plugin_api::check::Check;
use rb_plugin_api::feature::Feature;
use rb_plugin_api::{CheckContext, Failure, FailureKind, Finding, PluginHost, RuleInfo, codes};
use serde_json::{Value, json};

pub mod checks;
pub mod export;
pub mod fluid;
pub mod inventory;
pub mod page;
pub mod preview;
pub mod rendered;
pub mod rules;
pub mod run;
pub mod settings;
pub mod source;
pub mod tool;

#[cfg(test)]
mod fake;

#[cfg(target_arch = "wasm32")]
mod guest;

/// The plugin's id.
pub const PLUGIN_ID: &str = "fonts";
/// The panel the command `panel` shows.
pub const PANEL_ID: &str = "fonts";
/// The answer key asking the host to show a panel of the plugin.
pub const OPEN_KEY: &str = "$open";

/// The i18n key `plugin.fonts.<name>`.
#[must_use]
pub fn key(name: &str) -> String {
    format!("plugin.{PLUGIN_ID}.{name}")
}

/// The plugin: no state between calls (a store lives for one call).
#[derive(Clone, Copy, Debug, Default)]
pub struct Fonts;

impl Check for Fonts {
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

impl Feature for Fonts {
    fn run_command(
        &mut self,
        host: &mut dyn PluginHost,
        command: &str,
        args: &str,
    ) -> Result<String, Failure> {
        tool::parse(args)?;
        match command {
            "panel" => answer(Ok(json!({ OPEN_KEY: { "panel": PANEL_ID } }))),
            "check" => answer(tool::check_command(host)),
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
            "list" => answer(tool::list(host, &args)),
            "check" => answer(tool::check(host, &args)),
            "mark" => answer(tool::mark(host, &args)),
            "export" => answer(tool::export(host, &args)),
            "preview" => answer(preview::bytes(host, &args)),
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
