//! rBrowser's first-party coverage check (RB-080-07, `docs/plugins.md`): unused CSS and
//! JavaScript per breakpoint. CSS rule usage tracking and V8's precise coverage run in every
//! Chromium view at once over the host's CDP (`measure`), the byte ranges become used and unused
//! units per file and rule (`resolve`, `ranges`), the views are compared (`findings`: a file
//! mostly unused everywhere, a rule only some breakpoints use, a function no view ran), and the
//! result is kept in `.rbrowser/out/coverage/` for the check run, the panel and the export. The
//! logic is native Rust over [`PluginHost`], tested against fakes; `src/guest.rs` translates it
//! for the component.

use rb_plugin_api::check::Check;
use rb_plugin_api::feature::Feature;
use rb_plugin_api::{CheckContext, Failure, FailureKind, Finding, PluginHost, RuleInfo, codes};
use serde_json::{Value, json};

pub mod answer;
pub mod css_rules;
pub mod detail;
pub mod export;
pub mod findings;
pub mod measure;
pub mod model;
pub mod page;
pub mod ranges;
pub mod resolve;
pub mod settings;
pub mod store;
pub mod tool;

#[cfg(test)]
mod fake;

#[cfg(target_arch = "wasm32")]
mod guest;

/// The plugin's id.
pub const PLUGIN_ID: &str = "coverage";
/// The panel the commands open.
pub const PANEL_ID: &str = "coverage";
/// The answer key asking the host to show a panel of the plugin.
pub const OPEN_KEY: &str = "$open";

/// The i18n key `plugin.coverage.<name>`.
#[must_use]
pub fn key(name: &str) -> String {
    format!("plugin.{PLUGIN_ID}.{name}")
}

/// The plugin: no state between calls (a store lives for one call; the session is a file).
#[derive(Clone, Copy, Debug, Default)]
pub struct Coverage;

/// The check run: the findings of the last measurement for the views that still show the page
/// it measured. A check run measures nothing itself - a load run reloads every view, which a
/// `checks_run` must not do behind the person's back; `plugin.coverage.run_load` measures.
fn check(host: &mut dyn PluginHost, ctx: &CheckContext) -> Result<Vec<Finding>, Failure> {
    let Some(mut run) = store::last(host) else {
        host.log(
            "info",
            "coverage: no measurement yet (plugin.coverage.run_load)",
        );
        return Ok(Vec::new());
    };
    let current = host.views()?;
    // The one view a run names, of another engine, is never measured: the runner shows it
    // skipped, not "nothing found" (RB-080-98).
    if let Some(view) = ctx
        .view
        .and_then(|id| current.iter().find(|view| view.id == id))
    {
        view.require_chromium("cdp")?;
    }
    let page = |url: &str| url.split_once('#').map_or(url, |(page, _)| page).to_owned();
    run.views.retain(|view| {
        current
            .iter()
            .any(|now| now.id == view.view && page(&now.url) == page(&view.url))
    });
    let settings = settings::Settings::read(host);
    let texts = tool::texts(host, &run);
    Ok(findings::findings(
        &run, &settings, &ctx.rules, ctx.view, &texts,
    ))
}

impl Check for Coverage {
    fn describe(&self) -> Vec<RuleInfo> {
        findings::describe()
    }

    fn run(
        &mut self,
        host: &mut dyn PluginHost,
        ctx: &CheckContext,
    ) -> Result<Vec<Finding>, Failure> {
        check(host, ctx)
    }
}

fn answer(value: Result<Value, Failure>) -> Result<String, Failure> {
    value.map(|value| value.to_string())
}

impl Feature for Coverage {
    fn run_command(
        &mut self,
        host: &mut dyn PluginHost,
        command: &str,
        args: &str,
    ) -> Result<String, Failure> {
        tool::parse(args)?;
        match command {
            "run_load" => answer(tool::run_load(host)),
            "start" => answer(tool::start(host, &json!({})).map(|run| {
                let views = run["views"].as_array().map_or(0, Vec::len);
                tool::opened(json!({ "views": views }))
            })),
            "stop" => answer(tool::stop(host).map(|(summary, found)| {
                tool::report(host, &found);
                let files = summary["files"].as_array().map_or(0, Vec::len);
                tool::opened(json!({ "files": files, "findings": found.len() }))
            })),
            "export" => answer(tool::export(host).map(|written| {
                let mut answer = written;
                answer["$post"] = json!([{ "target": "panel", "message": { "exported": answer["written"].clone() } }]);
                answer
            })),
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
            "start" => answer(tool::start(host, &args)),
            "stop" => answer(tool::stop(host).map(|(summary, _)| summary)),
            "file" => answer(tool::file(host, &args)),
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

#[cfg(test)]
#[path = "fixture_tests.rs"]
mod fixture_tests;
