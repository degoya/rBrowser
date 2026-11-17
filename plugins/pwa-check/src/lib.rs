//! rBrowser's first-party PWA check (RB-080-08, `docs/plugins.md`): a `check-plugin` that holds
//! the page a view shows to what an installable Progressive Web App needs - a linked, reachable,
//! valid web app manifest with a name, a start URL inside its scope, an app display mode and
//! icons of 192 and 512 px that load and are what they claim; a service worker that is active,
//! controls the page and handles `fetch`; the start URL in its cache; Chromium's own
//! installability errors; `theme-color` and the iOS `apple-touch-icon`.
//!
//! The collector (`src/collect.js`) reads the page through `evaluate` in the plugin's isolated
//! world and only reads; the manifest, its icons, the start URL and the service worker's script
//! come over `http` from the page's own origin; the installability verdict is CDP
//! `Page.getInstallabilityErrors`. The logic is native Rust over [`PluginHost`], tested against
//! fakes; `src/guest.rs` translates it for the component.

use rb_plugin_api::check::Check;
use rb_plugin_api::feature::Feature;
use rb_plugin_api::{CheckContext, Failure, FailureKind, Finding, PluginHost, RuleInfo, codes};
use serde_json::{Value, json};

pub mod audit;
pub mod base64;
pub mod collect;
pub mod fetch;
pub mod image;
pub mod manifest;
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
pub const PLUGIN_ID: &str = "pwa-check";
/// The panel the command `panel` shows.
pub const PANEL_ID: &str = "pwa";
/// The answer key asking the host to show a panel of the plugin.
pub const OPEN_KEY: &str = "$open";
/// The answer key carrying posts to the plugin's panels.
pub const POST_KEY: &str = "$post";

/// The i18n key `plugin.pwa-check.<name>`.
#[must_use]
pub fn key(name: &str) -> String {
    format!("plugin.{PLUGIN_ID}.{name}")
}

/// The plugin: no state between calls (a store lives for one call).
#[derive(Clone, Copy, Debug, Default)]
pub struct PwaCheck;

impl Check for PwaCheck {
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

impl Feature for PwaCheck {
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
            "check" => answer(tool::check(host, &args)),
            "icon" => answer(tool::icon(host, &args)),
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
