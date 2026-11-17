//! rBrowser's first-party SEO check (RB-060-08, `docs/plugins.md`): a `check-plugin` that holds
//! a page to the essentials - status and redirects, indexability, canonical, title and
//! description, headings, `lang`, viewport and charset, Open Graph, JSON-LD, the sitemap in
//! `robots.txt` and the alt text group `seo.images` - over three sources: the raw response
//! (`http`), the raw HTML and the rendered DOM of a view (`evaluate`), whose differences are
//! findings of their own. A mini crawl checks the pages of the origin; the MCP tool `seo_check`
//! writes a report into `.rbrowser/out/seo/`. RB-060-09 adds the language set (`hreflang`, the
//! scope `hreflang`: the three sources, the codes, the targets, reciprocity) and the parity of
//! the views (`parity`, the argument `parity`). RB-080-11 adds the social previews (`social`:
//! the MCP tool `seo_social_preview` and the panel), the share image and Twitter card rules, and
//! the groups `wellknown`, `microdata` and `head`.
//!
//! The logic is native Rust over [`PluginHost`], tested against `MemoryHost`; `src/guest.rs`
//! translates it into the WIT contract for the component.

use rb_plugin_api::check::Check;
use rb_plugin_api::feature::Feature;
use rb_plugin_api::{CheckContext, Failure, FailureKind, Finding, PluginHost, RuleInfo, codes};
use serde::Deserialize;

pub mod audit;
pub mod config;
pub mod fetch;
pub mod hreflang;
pub mod i18n;
pub mod page;
pub mod parity;
pub mod path;
pub mod rendered;
pub mod report;
pub mod robots;
pub mod rules;
pub mod run;
pub mod site;
pub mod sitemap;
pub mod social;
pub mod tool;
pub mod url;

#[cfg(test)]
mod testing;

#[cfg(target_arch = "wasm32")]
mod guest;

/// The plugin: no state between calls (a store lives for one call).
#[derive(Clone, Copy, Debug, Default)]
pub struct Seo;

/// The arguments `checks_run` may hand a check: `{"scope": "crawl"}` crawls, `"hreflang"`
/// checks the language set; `{"parity": true}` compares the views.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct CheckArgs {
    scope: Option<String>,
    parity: bool,
}

impl Check for Seo {
    fn describe(&self) -> Vec<RuleInfo> {
        rules::describe()
    }

    fn run(
        &mut self,
        host: &mut dyn PluginHost,
        ctx: &CheckContext,
    ) -> Result<Vec<Finding>, Failure> {
        let value: serde_json::Value =
            serde_json::from_str(&ctx.args).unwrap_or(serde_json::Value::Null);
        let args: CheckArgs = serde_json::from_value(value.clone()).unwrap_or_default();
        let config = config::Config::read(host, &value);
        let request = run::Request {
            url: ctx.url.clone(),
            scope: match args.scope.as_deref() {
                Some("crawl") => run::Scope::Crawl,
                Some("hreflang") => run::Scope::Hreflang,
                _ => run::Scope::Page,
            },
            view: ctx.view,
            rules: ctx.rules.clone(),
            parity: args.parity,
        };
        Ok(run::run(host, &config, &request)?.findings)
    }
}

impl Feature for Seo {
    fn run_command(
        &mut self,
        host: &mut dyn PluginHost,
        command: &str,
        args: &str,
    ) -> Result<String, Failure> {
        match command {
            "check" => tool::check(host, args),
            "hreflang" => tool::check(host, &tool::with(args, "scope", "hreflang".into())),
            "parity" => tool::check(host, &tool::with(args, "parity", true.into())),
            "check_wellknown" => tool::check(
                host,
                &tool::with(args, "groups", serde_json::json!(["wellknown"])),
            ),
            "social_preview" => social_preview_command(host, args),
            _ => Err(unknown(command)),
        }
    }

    fn call_tool(
        &mut self,
        host: &mut dyn PluginHost,
        tool: &str,
        args: &str,
    ) -> Result<String, Failure> {
        match tool {
            "check" => tool::check(host, args),
            "social_preview" => social::tool::preview(host, args),
            _ => Err(unknown(tool)),
        }
    }
}

/// `plugin.seo.social_preview` (`K G`): the previews of the page, the SEO panel opened at them
/// (`$open`) and told to show the page anew (`$post`).
fn social_preview_command(host: &mut dyn PluginHost, args: &str) -> Result<String, Failure> {
    let preview: serde_json::Value = serde_json::from_str(&social::tool::preview(host, args)?)
        .map_err(|_| Failure::coded(FailureKind::BadData, codes::BAD_OUTPUT))?;
    let answer = serde_json::json!({
        "url": preview["url"],
        "cards": preview["cards"].as_array().map_or(0, Vec::len),
        "$open": { "panel": PANEL },
        "$post": [{ "target": "panel", "message": { "type": "preview", "url": preview["url"] } }],
    });
    Ok(answer.to_string())
}

/// The id of the SEO panel (`[[panels]]`).
pub const PANEL: &str = "seo";

fn unknown(name: &str) -> Failure {
    Failure::coded(FailureKind::NotFound, codes::UNKNOWN_CONTRIBUTION).with_param("name", name)
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
