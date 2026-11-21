//! rBrowser's first-party security header check (RB-093-03, `docs/plugins.md`): the response
//! headers of a page's main document and the cookies it sets, held to the common rules - CSP
//! (missing, `'unsafe-inline'`/`'unsafe-eval'`, wildcards, no `object-src`/`base-uri`), HSTS
//! (missing, `max-age` under `hstsMinAge`; never over `http:` or on a loopback host), frame
//! protection, `X-Content-Type-Options`, `Referrer-Policy`, `Permissions-Policy`, CORS `*` with
//! credentials, cookies without `Secure`/`HttpOnly`/`SameSite` and broken `__Host-`/`__Secure-`
//! prefixes - and compared across the project's environments (header x environment).
//!
//! The document is asked for once per URL through the host's `http` capability ([`fetch`]):
//! plugins receive no CDP events, and no CDP method of the plugins' allowlist answers a response's
//! headers. The cookies a view holds for its page come through CDP `Network.getCookies`. Cookie
//! values are dropped where they are read ([`cookies`]). Headers do not depend on the
//! breakpoint: one finding per URL and environment, no view.
//!
//! A `check-plugin` for `checks_run` and the findings panel, a `feature-plugin` for the MCP tools
//! `headers_check` and `headers_compare`, the commands and the comparison panel. The logic is
//! native Rust over [`PluginHost`], tested against `MemoryHost`; `src/guest.rs` translates it
//! for the component.

use rb_plugin_api::check::Check;
use rb_plugin_api::feature::Feature;
use rb_plugin_api::{CheckContext, Failure, FailureKind, Finding, PluginHost, RuleInfo, codes};
use serde_json::Value;

pub mod audit;
pub mod compare;
pub mod cookies;
pub mod csp;
pub mod date;
pub mod environments;
pub mod fetch;
pub mod headers;
pub mod levels;
pub mod page;
pub mod rules;
pub mod run;
pub mod settings;
pub mod tool;

#[cfg(test)]
mod testing;

#[cfg(target_arch = "wasm32")]
mod guest;

/// The plugin's id.
pub const PLUGIN_ID: &str = "security-headers";
/// The comparison panel.
pub const PANEL_ID: &str = "security-headers";
/// The answer key asking the host to show a panel of the plugin.
pub const OPEN_KEY: &str = "$open";
/// The answer key carrying posts to the plugin's panels.
pub const POST_KEY: &str = "$post";

/// The i18n key `plugin.security-headers.<name>`.
#[must_use]
pub fn key(name: &str) -> String {
    format!("plugin.{PLUGIN_ID}.{name}")
}

/// The plugin: no state between calls.
#[derive(Clone, Copy, Debug, Default)]
pub struct SecurityHeaders;

impl Check for SecurityHeaders {
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

impl Feature for SecurityHeaders {
    fn run_command(
        &mut self,
        host: &mut dyn PluginHost,
        command: &str,
        args: &str,
    ) -> Result<String, Failure> {
        let args = tool::parse(args)?;
        match command {
            "check" => answer(tool::check_command(host, &args)),
            "compare_environments" => answer(Ok(tool::compare_command())),
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
            "compare" => answer(tool::compare(host, &args)),
            _ => Err(unknown(tool)),
        }
    }
}

fn unknown(name: &str) -> Failure {
    Failure::coded(FailureKind::NotFound, codes::UNKNOWN_CONTRIBUTION)
        .with_param("name", name.chars().take(64).collect::<String>())
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
