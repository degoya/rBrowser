//! rBrowser's first-party outline (RB-070-03, `docs/plugins.md`): the structure of the page in
//! every view side by side - headings, landmarks, links, images, form fields, the focus order,
//! the accessibility tree and the person's own selectors - with what shows only in some views
//! marked, a broken-link check of the current page, and findings for `checks_run`.
//!
//! The collector (`web/src/collect.ts`, built into `src/collect.js`) reads each view's document
//! through `evaluate` in the plugin's isolated world; the accessibility tree and the mark come
//! over CDP (`Accessibility`, `DOM`, `Overlay`); the links over `http`. The logic is native Rust
//! over [`PluginHost`], tested against fakes; `src/guest.rs` translates it for the component.

use rb_plugin_api::check::Check;
use rb_plugin_api::feature::Feature;
use rb_plugin_api::{CheckContext, Failure, FailureKind, Finding, PluginHost, RuleInfo, codes};
use serde_json::{Value, json};

pub mod axtree;
pub mod budget;
pub mod links;
pub mod mark;
pub mod merge;
pub mod page;
pub mod rules;
pub mod run;
pub mod settings;
pub mod tool;
pub mod url;

#[cfg(test)]
mod fake;

#[cfg(target_arch = "wasm32")]
mod guest;

/// The plugin's id.
pub const PLUGIN_ID: &str = "outline";
/// The panel the command `panel` shows.
pub const PANEL_ID: &str = "outline";
/// The answer key asking the host to show a panel of the plugin (as the console plugin's).
pub const OPEN_KEY: &str = "$open";

/// The i18n key `plugin.outline.<name>`.
#[must_use]
pub fn key(name: &str) -> String {
    format!("plugin.{PLUGIN_ID}.{name}")
}

/// The plugin: no state between calls (a store lives for one call).
#[derive(Clone, Copy, Debug, Default)]
pub struct Outline;

impl Check for Outline {
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

impl Feature for Outline {
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
            "get" => answer(tool::get(host, &args)),
            "check_links" => answer(tool::check_links(host, &args)),
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
