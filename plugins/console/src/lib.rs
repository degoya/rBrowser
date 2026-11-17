//! The shared console (RB-060-02, `docs/plugins.md`): one console over every view.
//!
//! - **`grouped`** (MCP `sharedconsole_grouped`, the panel's list): the views' console messages,
//!   uncaught exceptions and failed fetch/XHR requests ([`feed`], handed by the host) deduplicated
//!   ([`group`], the rule in [`normalize`]), each with the views that reported it and how often;
//!   `clear: true` clears after answering. The command `clear` (`Ctrl+Shift+Space D X`) clears.
//! - **`exec`** ([`exec`]): an expression run in the views, one result per distinct value. The
//!   command (`Ctrl+Shift+Space D E`, the panel's command line) always runs; the MCP tool
//!   `sharedconsole_exec` only while a person allows it (`console.mcpExec`, protected).
//! - **The check** ([`findings`]): `js-error` and `console-error` as findings for `checks_run`.
//! - **`panel`** (`Ctrl+Shift+Space P C`) asks the host to show the panel `console`.
//!
//! The logic is plain JSON on the native traits and tested natively; `guest.rs` translates it
//! into the contract's vocabulary and exists on `wasm32` alone.

#[cfg(target_arch = "wasm32")]
mod guest;

pub mod config;
pub mod exec;
pub mod feed;
pub mod findings;
pub mod group;
pub mod normalize;

#[cfg(test)]
mod fake_host;

use std::collections::{BTreeMap, BTreeSet};

use rb_plugin_api::check::Check;
use rb_plugin_api::feature::Feature;
use rb_plugin_api::{
    CheckContext, Failure, FailureKind, Finding, PluginHost, RuleInfo, ViewId, codes,
};
use serde_json::{Value, json};

use config::Settings;
use exec::Target;
use feed::{FeedOptions, Level};
use group::Filter;

/// The plugin's id: the prefix of its keys.
pub const PLUGIN_ID: &str = "console";
/// The panel the command `panel` shows.
pub const PANEL_ID: &str = "console";
/// The answer key asking the host to show a panel of the plugin.
pub const OPEN_KEY: &str = "$open";

/// The tool `grouped`.
pub const GROUPED: &str = "grouped";
/// The tool and command `exec`.
pub const EXEC: &str = "exec";
/// The command `clear`.
pub const CLEAR: &str = "clear";
/// The command `panel`.
pub const PANEL: &str = "panel";

/// The i18n key `plugin.console.<name>`.
#[must_use]
pub fn key(name: &str) -> String {
    format!("plugin.{PLUGIN_ID}.{name}")
}

pub(crate) fn coded(kind: FailureKind, name: &str) -> Failure {
    Failure::coded(kind, &key(name))
}

pub(crate) fn bad_args(reason: &str) -> Failure {
    coded(FailureKind::BadData, "errors.bad_args").with_param("reason", normalize::cut(reason, 200))
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

fn feed_options(settings: &Settings, cleared_at: f64) -> FeedOptions {
    FeedOptions {
        buffer_size: settings.buffer_size,
        clear_on_navigate: settings.clear_on_navigate,
        network: settings.network,
        cleared_at,
        rbrowser: settings.show_rbrowser,
    }
}

fn filter(args: &Value, settings: &Settings) -> Result<Filter, Failure> {
    let levels = match &args["levels"] {
        Value::Null => settings.levels.clone(),
        Value::Array(names) => names
            .iter()
            .map(|name| name.as_str().and_then(Level::parse))
            .collect::<Option<BTreeSet<Level>>>()
            .ok_or_else(|| bad_args("levels"))?,
        _ => return Err(bad_args("levels")),
    };
    let mut views: Option<BTreeSet<ViewId>> = match &args["views"] {
        Value::Null => None,
        Value::Array(ids) => Some(
            ids.iter()
                .map(|id| id.as_u64().and_then(|id| ViewId::try_from(id).ok()))
                .collect::<Option<_>>()
                .ok_or_else(|| bad_args("views"))?,
        ),
        _ => return Err(bad_args("views")),
    };
    // MCP's `target` (a view id) narrows like `views`.
    if let Some(target) = args["target"]
        .as_u64()
        .and_then(|t| ViewId::try_from(t).ok())
    {
        views.get_or_insert_with(BTreeSet::new).insert(target);
    }
    let search = match &args["search"] {
        Value::Null => None,
        Value::String(text) if text.trim().is_empty() => None,
        Value::String(text) => Some(normalize::cut(text.trim(), group::MAX_SEARCH_CHARS)),
        _ => return Err(bad_args("search")),
    };
    Ok(Filter {
        levels,
        views,
        search,
    })
}

fn view_list(host: &mut dyn PluginHost) -> Vec<Value> {
    host.views()
        .unwrap_or_default()
        .into_iter()
        .map(|view| {
            json!({ "id": view.id, "label": view.label, "breakpoint": view.breakpoint,
                    "engine": view.engine, "width": view.width })
        })
        .collect()
}

/// `grouped`: the page of groups `args` asks for, the views, whether the host captures, and -
/// with `clear: true` - the clear mark set after reading (posted to the panel too).
///
/// # Errors
///
/// Arguments that do not read.
pub fn grouped(host: &mut dyn PluginHost, args: &Value) -> Result<Value, Failure> {
    let settings = Settings::read(host);
    let cleared_at = config::cleared_at(host);
    let feed = feed::read(args, &feed_options(&settings, cleared_at)).map_err(|e| bad_args(&e))?;
    let filter = filter(args, &settings)?;
    let limit = match &args["limit"] {
        Value::Null => group::DEFAULT_LIMIT,
        value => value
            .as_u64()
            .and_then(|n| usize::try_from(n).ok())
            .filter(|n| (1..=group::MAX_LIMIT).contains(n))
            .ok_or_else(|| bad_args("limit"))?,
    };
    let mut answer = group::page(&group::group(&feed.entries), &filter, limit);
    answer["captured"] = json!(feed.captured);
    answer["views"] = json!(view_list(host));
    if args["clear"] == json!(true) {
        let mark = config::clear(host, feed.newest);
        answer["clearedAt"] = json!(mark);
        answer["$post"] = json!([{ "target": "panel", "message": { "cleared": mark } }]);
    }
    Ok(answer)
}

/// The command `clear`: marks everything captured so far cleared and tells the panel.
pub fn clear(host: &mut dyn PluginHost, args: &Value) -> Value {
    let settings = Settings::read(host);
    let newest = feed::read(args, &feed_options(&settings, 0.0))
        .ok()
        .filter(|feed| feed.captured)
        .map_or(f64::INFINITY, |feed| feed.newest);
    let mark = config::clear(host, newest);
    json!({ "clearedAt": mark, "$post": [{ "target": "panel", "message": { "cleared": mark } }] })
}

/// `exec` from the command (a person: the panel, the palette): `expression` in the views of
/// `target` (`"all"`, `"active"`, a view id), `console.execTarget` without one.
///
/// # Errors
///
/// [`exec::run`]'s.
pub fn exec_command(host: &mut dyn PluginHost, args: &Value) -> Result<Value, Failure> {
    let settings = Settings::read(host);
    let target = Target::parse(&args["target"], settings.exec_target)?;
    let expression = match &args["expression"] {
        Value::String(text) => text.as_str(),
        Value::Null => "",
        _ => return Err(bad_args("expression")),
    };
    exec::run(host, expression, target)
}

/// `exec` from MCP: refused unless a person allowed it (`console.mcpExec`); then as the command,
/// in the views of MCP's `target` (`"all"` or a view id).
///
/// # Errors
///
/// `plugin.console.errors.mcp_exec_off`, or [`exec_command`]'s.
pub fn exec_tool(host: &mut dyn PluginHost, args: &Value) -> Result<Value, Failure> {
    if !Settings::read(host).mcp_exec {
        return Err(coded(FailureKind::AccessDenied, "errors.mcp_exec_off"));
    }
    let mut args = args.clone();
    if args["target"] == json!("active") {
        return Err(bad_args("target"));
    }
    if args["target"].is_null() {
        args["target"] = json!("all");
    }
    exec_command(host, &args)
}

/// The plugin.
pub struct Console;

fn answer(value: Result<Value, Failure>) -> Result<String, Failure> {
    value.map(|value| value.to_string())
}

impl Feature for Console {
    fn run_command(
        &mut self,
        host: &mut dyn PluginHost,
        command: &str,
        args: &str,
    ) -> Result<String, Failure> {
        let args = parse(args)?;
        match command {
            PANEL => answer(Ok(json!({ OPEN_KEY: { "panel": PANEL_ID } }))),
            CLEAR => answer(Ok(clear(host, &args))),
            EXEC => answer(exec_command(host, &args)),
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
            GROUPED => answer(grouped(host, &args)),
            EXEC => answer(exec_tool(host, &args)),
            other => Err(unknown(other)),
        }
    }
}

impl Check for Console {
    fn describe(&self) -> Vec<RuleInfo> {
        findings::rules()
    }

    fn run(
        &mut self,
        host: &mut dyn PluginHost,
        ctx: &CheckContext,
    ) -> Result<Vec<Finding>, Failure> {
        let args = parse(&ctx.args)?;
        let settings = Settings::read(host);
        let cleared_at = config::cleared_at(host);
        let feed =
            feed::read(&args, &feed_options(&settings, cleared_at)).map_err(|e| bad_args(&e))?;
        let urls: BTreeMap<ViewId, String> = host
            .views()
            .unwrap_or_default()
            .into_iter()
            .map(|view| (view.id, view.url))
            .collect();
        let groups = group::group(&feed.entries);
        Ok(findings::findings(&groups, ctx.view, &ctx.rules, &urls))
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
