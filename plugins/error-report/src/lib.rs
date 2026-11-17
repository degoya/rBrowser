//! The error report (RB-080-09, `docs/plugins.md`): every view's JS exceptions, 404/410 and 5xx
//! resources, mixed content, CSP violations and deprecations as one report, deduplicated over
//! the views - an image that 404s only in the mobile breakpoint is one entry "only in views:
//! mobile", the same exception in four views one entry with four view dots.
//!
//! - **`get`** (MCP `errorreport_get`, the panel's list): the report as JSON ([`report`]) from
//!   the host's capture ([`feed`], classified by [`classify`]); `reload: true` reloads the views
//!   and waits for network idle instead ([`reload`]), the next call reports the fresh load.
//! - **`run`** (command "Report now", `Ctrl+Shift+Space K E`): publishes the report -
//!   `.rbrowser/out/error-report/report.json` and `report.html` ([`html`]) and the findings into
//!   the findings panel - and shows the panel; with `reloadBeforeReport` it reloads first and the
//!   next call publishes.
//! - **`clear`** (MCP `errorreport_clear`, `Ctrl+Shift+Space K Z`): what was reported so far
//!   leaves the report ([`config::State`]).
//! - **`panel`** (`Ctrl+Shift+Space P E`) shows the panel `report`.
//! - **The check** ([`findings`]): one rule per category for `checks_run`.
//!
//! The logic is plain JSON on the native traits and tested natively; `guest.rs` translates it
//! into the contract's vocabulary and exists on `wasm32` alone.

#[cfg(target_arch = "wasm32")]
mod guest;

pub mod classify;
pub mod config;
pub mod feed;
pub mod findings;
pub mod html;
pub mod normalize;
pub mod reload;
pub mod report;

#[cfg(test)]
mod fake_host;

use rb_plugin_api::check::Check;
use rb_plugin_api::feature::Feature;
use rb_plugin_api::{
    CheckContext, Failure, FailureKind, Finding, PluginHost, RuleInfo, ViewId, ViewInfo, codes,
};
use serde_json::{Value, json};

use config::{Settings, State};
use report::Report;

/// The plugin's id: the prefix of its keys.
pub const PLUGIN_ID: &str = "error-report";
/// The panel the command `panel` shows.
pub const PANEL_ID: &str = "report";
/// The answer key asking the host to show a panel of the plugin.
pub const OPEN_KEY: &str = "$open";
/// The tool `get`.
pub const GET: &str = "get";
/// The tool and command `clear`.
pub const CLEAR: &str = "clear";
/// The command `run`.
pub const RUN: &str = "run";
/// The command `panel`.
pub const PANEL: &str = "panel";
/// The JSON report, relative to the plugin's out folder.
pub const REPORT_JSON: &str = "report.json";
/// The HTML report, relative to the plugin's out folder.
pub const REPORT_HTML: &str = "report.html";
/// The entries `get` answers without a `limit`.
pub const DEFAULT_LIMIT: usize = 500;
/// The most entries `get` answers.
pub const MAX_LIMIT: usize = 1000;

/// The i18n key `plugin.error-report.<name>`.
#[must_use]
pub fn key(name: &str) -> String {
    format!("plugin.{PLUGIN_ID}.{name}")
}

fn bad_args(reason: &str) -> Failure {
    Failure::coded(FailureKind::BadData, &key("errors.bad_args"))
        .with_param("reason", normalize::cut(reason, 200))
}

fn unknown(name: &str) -> Failure {
    Failure::coded(FailureKind::NotFound, codes::UNKNOWN_CONTRIBUTION)
        .with_param("name", normalize::cut(name, 64))
}

/// `args` as a JSON object (`{}` for an empty text).
///
/// # Errors
///
/// No JSON object.
pub fn parse(args: &str) -> Result<Value, Failure> {
    if args.trim().is_empty() {
        return Ok(json!({}));
    }
    match serde_json::from_str::<Value>(args) {
        Ok(value) if value.is_object() => Ok(value),
        _ => Err(bad_args("arguments")),
    }
}

/// MCP's `target` when it names one view.
fn target(args: &Value) -> Option<ViewId> {
    args["target"]
        .as_u64()
        .and_then(|id| ViewId::try_from(id).ok())
}

fn flag(args: &Value, name: &str) -> Result<Option<bool>, Failure> {
    match &args[name] {
        Value::Null => Ok(None),
        Value::Bool(value) => Ok(Some(*value)),
        _ => Err(bad_args(name)),
    }
}

/// The report of the capture in `args` with the settings and state in effect.
///
/// # Errors
///
/// A capture that does not read.
pub fn report_of(host: &mut dyn PluginHost, args: &Value) -> Result<Report, Failure> {
    let settings = Settings::read(host);
    let state = State::read(host);
    let feed = feed::read(args).map_err(|e| bad_args(&e))?;
    let views: Vec<ViewInfo> = host.views().unwrap_or_default();
    Ok(report::build(&feed, &views, &settings, state.cleared_at))
}

/// Writes the report's files and reports its findings; answers what was published.
fn publish(host: &mut dyn PluginHost, report: &Report) -> Value {
    let now = host.now_unix_seconds();
    let mut json_report = report::to_json(report, None, usize::MAX);
    json_report["generatedAt"] = json!(now);
    let mut written = true;
    for (file, text) in [
        (REPORT_JSON, json_report.to_string()),
        (REPORT_HTML, html::render(report, now)),
    ] {
        if let Err(failure) = host.out_write(file, text.as_bytes()) {
            written = false;
            let code = failure.code.unwrap_or_default();
            host.log("warn", &format!("{file} was not written: {code}"));
        }
    }
    let found = findings::findings(report, None, &[]);
    let reported = match host.report_findings(&found) {
        Ok(()) => true,
        Err(failure) => {
            let code = failure.code.unwrap_or_default();
            host.log("warn", &format!("the findings were not reported: {code}"));
            false
        }
    };
    json!({ "written": written, "files": [REPORT_JSON, REPORT_HTML],
            "findings": found.len(), "findingsReported": reported })
}

/// Reloads the views of `args`' target (every Chromium view without one) and marks the report
/// pending: the next call publishes the fresh load.
fn reload_views(host: &mut dyn PluginHost, args: &Value) -> Value {
    let views: Vec<ViewId> = match target(args) {
        Some(view) => vec![view],
        None => host
            .views()
            .unwrap_or_default()
            .into_iter()
            .filter(|view| view.engine == "chromium")
            .map(|view| view.id)
            .collect(),
    };
    let outcomes = reload::reload(host, &views);
    let mut state = State::read(host);
    state.pending = true;
    state.write(host);
    let outcomes: Vec<Value> = outcomes
        .iter()
        .map(|o| json!({ "view": o.view, "settled": o.settled, "error": o.error }))
        .collect();
    json!({
        "reloaded": outcomes,
        "pending": true,
        "$post": [{ "target": "panel", "message": { "reloaded": views.len() } }],
    })
}

/// `get`: the report as JSON (the views of `target` only), or with `reload: true` the reload.
/// A report pending after a reload is published on the way.
///
/// # Errors
///
/// Arguments or a capture that do not read.
pub fn get(host: &mut dyn PluginHost, args: &Value) -> Result<Value, Failure> {
    if flag(args, "reload")? == Some(true) {
        return Ok(reload_views(host, args));
    }
    let limit = match &args["limit"] {
        Value::Null => DEFAULT_LIMIT,
        value => value
            .as_u64()
            .and_then(|n| usize::try_from(n).ok())
            .filter(|n| (1..=MAX_LIMIT).contains(n))
            .ok_or_else(|| bad_args("limit"))?,
    };
    let report = report_of(host, args)?;
    let mut answer = report::to_json(&report, target(args), limit);
    let mut state = State::read(host);
    if state.pending {
        answer["published"] = publish(host, &report);
        state.pending = false;
        state.write(host);
    }
    Ok(answer)
}

/// `run` ("Report now"): publishes the report and shows the panel; reloads first when `reload`
/// (or `reloadBeforeReport`) says so.
///
/// # Errors
///
/// Arguments or a capture that do not read.
pub fn run(host: &mut dyn PluginHost, args: &Value) -> Result<Value, Failure> {
    let reload = flag(args, "reload")?.unwrap_or_else(|| Settings::read(host).reload_before_report);
    let mut answer = if reload {
        reload_views(host, args)
    } else {
        let report = report_of(host, args)?;
        let mut state = State::read(host);
        if state.pending {
            state.pending = false;
            state.write(host);
        }
        let mut answer = report::to_json(&report, None, 0);
        answer.as_object_mut().map(|o| o.remove("entries"));
        answer["published"] = publish(host, &report);
        answer["$post"] = json!([{ "target": "panel", "message": { "published": true } }]);
        answer
    };
    answer[OPEN_KEY] = json!({ "panel": PANEL_ID });
    Ok(answer)
}

/// `clear`: marks everything captured so far cleared and tells the panel.
pub fn clear(host: &mut dyn PluginHost, args: &Value) -> Value {
    let newest = feed::read(args).map_or(0.0, |feed| feed.newest());
    let mut state = State::read(host);
    state.cleared_at = config::clear_mark(host, newest);
    state.write(host);
    json!({ "clearedAt": state.cleared_at,
            "$post": [{ "target": "panel", "message": { "cleared": state.cleared_at } }] })
}

/// The plugin.
pub struct ErrorReport;

fn answer(value: Result<Value, Failure>) -> Result<String, Failure> {
    value.map(|value| value.to_string())
}

impl Feature for ErrorReport {
    fn run_command(
        &mut self,
        host: &mut dyn PluginHost,
        command: &str,
        args: &str,
    ) -> Result<String, Failure> {
        let args = parse(args)?;
        match command {
            PANEL => answer(Ok(json!({ OPEN_KEY: { "panel": PANEL_ID } }))),
            RUN => answer(run(host, &args)),
            CLEAR => answer(Ok(clear(host, &args))),
            other => Err(unknown(other)),
        }
    }

    fn call_tool(
        &mut self,
        host: &mut dyn PluginHost,
        tool: &str,
        args: &str,
    ) -> Result<String, Failure> {
        let args = parse(args)?;
        match tool {
            GET => answer(get(host, &args)),
            CLEAR => answer(Ok(clear(host, &args))),
            other => Err(unknown(other)),
        }
    }
}

impl Check for ErrorReport {
    fn describe(&self) -> Vec<RuleInfo> {
        findings::rules()
    }

    fn run(
        &mut self,
        host: &mut dyn PluginHost,
        ctx: &CheckContext,
    ) -> Result<Vec<Finding>, Failure> {
        let args = parse(&ctx.args)?;
        let report = report_of(host, &args)?;
        Ok(findings::findings(&report, ctx.view, &ctx.rules))
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
