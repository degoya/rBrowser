//! rBrowser's first-party consent pre-check (RB-094-05, `docs/plugins.md`): which requests to
//! other sites, cookies and localStorage keys a page has before any consent, and after "Reject
//! all" of its cookie banner once more - a technical pre-check for the GDPR and the TDDDG, no
//! legal advice.
//!
//! The tool and the command run in a fresh, empty session of their own that is discarded
//! afterwards: the core's consent probe (RB-094-09, capability `consent-probe`) - the plugin
//! answers the host's offer with the page and its collectors and is called again with the
//! capture `$consent` (`capture`, `tool`). A `checks_run`, offered no probe, reads the target
//! view as it is, read only, and says what that cannot show. The
//! banner detection and the "Reject all" click are the collector `src/collect.js` over the
//! bundled consent managers of `data/cmp.json`.
//!
//! A `check-plugin` for `checks_run` and the findings panel, a `feature-plugin` for the MCP tool
//! `consent_check`, the command `check` and the panel. The logic is native Rust over
//! [`PluginHost`], tested against `MemoryHost`; `src/guest.rs` translates it for the component.

use rb_plugin_api::check::Check;
use rb_plugin_api::feature::Feature;
use rb_plugin_api::{CheckContext, Failure, FailureKind, Finding, PluginHost, RuleInfo, codes};
use serde_json::Value;

pub mod analyze;
pub mod capture;
pub mod cmp;
pub mod rules;
pub mod settings;
pub mod site;
pub mod tool;

#[cfg(test)]
mod testing;

#[cfg(target_arch = "wasm32")]
mod guest;

/// The plugin's id.
pub const PLUGIN_ID: &str = "consent-check";
/// The plugin's panel.
pub const PANEL_ID: &str = "consent-check";
/// The answer key carrying posts to the plugin's panels.
pub const POST_KEY: &str = "$post";
/// The notice of every answer, the panel and every export, as an i18n key.
pub const NOTICE_KEY: &str = "plugin.consent-check.notice";
/// The notice in English, for the exports and the findings' data; without double quotes, so it
/// reads the same inside JSON text.
pub const NOTICE_TEXT: &str = "Technical pre-check, no legal advice: it shows what the page \
    loads and stores before a choice and after 'Reject all', not whether that is lawful.";

/// The i18n key `plugin.consent-check.<name>`.
#[must_use]
pub fn key(name: &str) -> String {
    format!("plugin.{PLUGIN_ID}.{name}")
}

/// The plugin: no state between calls.
#[derive(Clone, Copy, Debug, Default)]
pub struct ConsentCheck;

impl Check for ConsentCheck {
    fn describe(&self) -> Vec<RuleInfo> {
        rules::describe()
    }

    fn run(
        &mut self,
        host: &mut dyn PluginHost,
        ctx: &CheckContext,
    ) -> Result<Vec<Finding>, Failure> {
        let mut args = tool::parse(&ctx.args)?;
        // A checks run reads its view; only the tool and the command ask for a probe.
        args.remove(tool::ASK_KEY);
        if let Some(url) = &ctx.url {
            args.entry("url")
                .or_insert_with(|| Value::String(url.clone()));
        }
        tool::check(host, &args, &ctx.rules, ctx.view).map(|checked| checked.findings)
    }
}

fn answer(value: Result<Value, Failure>) -> Result<String, Failure> {
    value.map(|value| value.to_string())
}

impl Feature for ConsentCheck {
    fn run_command(
        &mut self,
        host: &mut dyn PluginHost,
        command: &str,
        args: &str,
    ) -> Result<String, Failure> {
        let args = tool::parse(args)?;
        match command {
            "check" => answer(tool::command(host, &args)),
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
            "check" => answer(tool::check(host, &args, &[], None).map(|checked| checked.answer)),
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
