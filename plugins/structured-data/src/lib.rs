//! rBrowser's first-party structured data panel (RB-090-96, `docs/plugins.md`): the schema.org
//! data of the page a view shows - JSON-LD, Microdata and RDFa - as one tree, held to the
//! schema.org vocabulary of a pinned release (unknown type, unknown property, a property not for
//! its type, a value of the wrong kind, a superseded property, broken JSON) and to the properties
//! the common rich results of Google Search need and recommend.
//!
//! The collector (`src/collect.js`) reads the rendered document through `evaluate` in the
//! plugin's isolated world; the readers, the vocabulary and the rich-result table are the shared
//! library `plugins/schema-org`, which the SEO check uses too. A `check-plugin` for `checks_run`
//! and the findings panel, a `feature-plugin` for the MCP tool `structureddata_inspect`, the
//! panel and its commands. The logic is native Rust over [`PluginHost`], tested against
//! `MemoryHost`; `src/guest.rs` translates it for the component.

use rb_plugin_api::check::Check;
use rb_plugin_api::feature::Feature;
use rb_plugin_api::{CheckContext, Failure, FailureKind, Finding, PluginHost, RuleInfo, codes};
use serde_json::{Value, json};

pub mod formats;
pub mod page;
pub mod rules;
pub mod run;
pub mod settings;
pub mod tool;
pub mod tree;
pub mod validate;

#[cfg(test)]
mod testing;

#[cfg(target_arch = "wasm32")]
mod guest;

/// The plugin's id.
pub const PLUGIN_ID: &str = "structured-data";
/// The panel the command `panel` shows.
pub const PANEL_ID: &str = "structured-data";
/// The answer key asking the host to show a panel of the plugin.
pub const OPEN_KEY: &str = "$open";
/// The answer key carrying posts to the plugin's panels.
pub const POST_KEY: &str = "$post";

/// The i18n key `plugin.structured-data.<name>`.
#[must_use]
pub fn key(name: &str) -> String {
    format!("plugin.{PLUGIN_ID}.{name}")
}

/// The plugin: no state between calls (a store lives for one call).
#[derive(Clone, Copy, Debug, Default)]
pub struct StructuredData;

impl Check for StructuredData {
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

impl Feature for StructuredData {
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
            "inspect" => answer(tool::inspect(host, &args)),
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
