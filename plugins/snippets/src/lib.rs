//! Snippets (RB-070-04, `docs/plugins.md`): JavaScript snippets of the person and the project,
//! run in every view at once or in the active one, the results joined per view.
//!
//! - **The catalogue** ([`catalog`]): the person's `snippets.library`, the project's
//!   `.rbrowser/snippets/*.js` and the panel's saved ones - the latter two only under workspace
//!   trust. Each snippet's header ([`header`]) names it and sets its world, target and timeout.
//! - **`list`** (MCP `snippets_list`, the panel's list), **`run`** (MCP `snippets_run`, the
//!   panel's runs; a stored snippet by `name`, or `code`) and **`save`** (MCP `snippets_save`,
//!   the panel's editor; [`library`]). `run` and `save` take code to run, so the host serves
//!   them over MCP only while `mcp.allowEvaluate` is on (`evaluates_input`).
//! - **Running** ([`run`], [`script`]): CDP `Runtime.evaluate` in the page's world (`main`) or
//!   the plugin's isolated world, with the snippet's timeout.
//! - **Commands**: `panel` (`Ctrl+Shift+Space P J`) and `run_last` (`Ctrl+Shift+Space D J`, the
//!   last snippet in the same views again, [`last`]).
//!
//! The logic is plain JSON on the native traits and tested natively; `guest.rs` translates it
//! into the contract's vocabulary and exists on `wasm32` alone.

#[cfg(target_arch = "wasm32")]
mod guest;

pub mod catalog;
pub mod config;
pub mod header;
pub mod last;
pub mod library;
pub mod run;
pub mod script;
pub mod text;

#[cfg(test)]
mod fake_host;

use rb_plugin_api::feature::Feature;
use rb_plugin_api::{Failure, FailureKind, PluginHost, ViewId, codes};
use serde_json::{Value, json};

use catalog::{MAX_CODE_CHARS, Source};
use config::Settings;
use header::{MAX_TIMEOUT_MS, MIN_TIMEOUT_MS, World};
use run::{Job, RunTarget, Target};

/// The plugin's id: the prefix of its keys.
pub const PLUGIN_ID: &str = "snippets";
/// The panel the command `panel` shows.
pub const PANEL_ID: &str = "snippets";
/// The answer key asking the host to show a panel of the plugin.
pub const OPEN_KEY: &str = "$open";

/// The tools.
pub const LIST: &str = "list";
pub const RUN: &str = "run";
pub const SAVE: &str = "save";
/// The commands.
pub const PANEL: &str = "panel";
pub const RUN_LAST: &str = "run_last";

/// The i18n key `plugin.snippets.<name>`.
#[must_use]
pub fn key(name: &str) -> String {
    format!("plugin.{PLUGIN_ID}.{name}")
}

pub(crate) fn coded(kind: FailureKind, name: &str) -> Failure {
    Failure::coded(kind, &key(name))
}

/// A view's outcome that is the plugin's own failure `name`.
pub(crate) fn coded_outcome(name: &str) -> Value {
    json!({ "ok": false, "type": "failure", "text": key(name), "timedOut": false })
}

pub(crate) fn bad_args(reason: &str) -> Failure {
    coded(FailureKind::BadData, "errors.bad_args").with_param("reason", text::cut(reason, 200))
}

fn unknown(name: &str) -> Failure {
    Failure::coded(FailureKind::NotFound, codes::UNKNOWN_CONTRIBUTION)
        .with_param("name", text::cut(name, 64))
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

/// `list`: the catalogue, with each snippet's code when `withCode` is true, and the views
/// (`id`, `label`, `breakpoint`, `engine`) and the active one for the panel's runs.
///
/// # Errors
///
/// `withCode` that is no boolean.
pub fn list(host: &mut dyn PluginHost, args: &Value) -> Result<Value, Failure> {
    let with_code = match &args["withCode"] {
        Value::Null => false,
        Value::Bool(with) => *with,
        _ => return Err(bad_args("withCode")),
    };
    let settings = Settings::read(host);
    let mut answer = catalog::load(host, &settings).to_json(with_code);
    let views: Vec<Value> = host
        .views()
        .unwrap_or_default()
        .into_iter()
        .map(|view| {
            json!({ "id": view.id, "label": view.label, "breakpoint": view.breakpoint,
                            "engine": view.engine })
        })
        .collect();
    answer["views"] = json!(views);
    answer["active"] = json!(host.active_view().ok().flatten());
    Ok(answer)
}

/// The code of an ad-hoc run (`code`), its header over the arguments' and the settings' world
/// and timeout.
fn ad_hoc<'a>(
    args: &Value,
    code: &'a str,
    settings: &Settings,
) -> Result<(Job<'a>, RunTarget), Failure> {
    if code.trim().is_empty() {
        return Err(coded(FailureKind::BadData, "errors.empty"));
    }
    if code.chars().count() > MAX_CODE_CHARS {
        return Err(
            coded(FailureKind::Limit, "errors.code_too_long").with_param("limit", MAX_CODE_CHARS)
        );
    }
    let header = header::parse(code)
        .map_err(|bad| coded(FailureKind::BadData, "errors.header").with_param("tag", bad.tag))?;
    let world = match &args["world"] {
        Value::Null => header.world.unwrap_or(settings.default_world),
        value => value
            .as_str()
            .and_then(World::parse)
            .ok_or_else(|| bad_args("world"))?,
    };
    let timeout_ms = match &args["timeoutMs"] {
        Value::Null => header.timeout_ms.unwrap_or(settings.default_timeout_ms),
        value => value
            .as_u64()
            .and_then(|ms| u32::try_from(ms).ok())
            .filter(|ms| (MIN_TIMEOUT_MS..=MAX_TIMEOUT_MS).contains(ms))
            .ok_or_else(|| bad_args("timeoutMs"))?,
    };
    Ok((
        Job {
            code,
            world,
            timeout_ms,
        },
        header.target.unwrap_or_default(),
    ))
}

/// The views to remember for a repeat: `remember` when given, else the run's.
fn remembered(args: &Value, views: &[ViewId]) -> Result<Vec<ViewId>, Failure> {
    match &args["remember"] {
        Value::Null => Ok(views.to_vec()),
        Value::Array(ids) if !ids.is_empty() && ids.len() <= last::MAX_VIEWS => ids
            .iter()
            .map(|id| id.as_u64().and_then(|id| ViewId::try_from(id).ok()))
            .collect::<Option<_>>()
            .ok_or_else(|| bad_args("remember")),
        _ => Err(bad_args("remember")),
    }
}

/// `run`: a stored snippet (`name`, optional `source`) or `code`, in the views of `target`
/// (`"all"`, `"active"`, a view id or a list; the snippet's `@target` without one). A stored
/// snippet's run is remembered for `run_last`, with the views of `remember` when the caller
/// ran it view by view (the panel).
///
/// # Errors
///
/// Arguments that do not read, a snippet that is not there or cannot run, no active view.
pub fn run_tool(host: &mut dyn PluginHost, args: &Value) -> Result<Value, Failure> {
    let settings = Settings::read(host);
    let known = host.views()?;
    if let Some(code) = args["code"].as_str() {
        let (job, default) = ad_hoc(args, code, &settings)?;
        let views = run::resolve(host, &known, &Target::parse(&args["target"], default)?)?;
        let mut answer = run::run(host, &known, &views, &job);
        answer["world"] = json!(job.world.as_str());
        answer["timeoutMs"] = json!(job.timeout_ms);
        return Ok(answer);
    }
    let name = args["name"].as_str().ok_or_else(|| bad_args("name"))?;
    let source = match &args["source"] {
        Value::Null => None,
        value => Some(
            value
                .as_str()
                .and_then(Source::parse)
                .ok_or_else(|| bad_args("source"))?,
        ),
    };
    let catalog = catalog::load(host, &settings);
    let snippet = catalog.find(source, name)?.clone();
    let views = run::resolve(
        host,
        &known,
        &Target::parse(&args["target"], snippet.target)?,
    )?;
    let remember = remembered(args, &views)?;
    let job = Job {
        code: &snippet.code,
        world: snippet.world,
        timeout_ms: snippet.timeout_ms,
    };
    let mut answer = run::run(host, &known, &views, &job);
    last::remember(
        host,
        &last::Last {
            source: snippet.source,
            name: snippet.name.clone(),
            views: remember,
        },
    );
    answer["snippet"] = json!({ "source": snippet.source.as_str(), "name": snippet.name });
    answer["world"] = json!(snippet.world.as_str());
    answer["timeoutMs"] = json!(snippet.timeout_ms);
    Ok(answer)
}

/// `run_last`: the last stored snippet again, in the same views; the result goes to the panel
/// too.
///
/// # Errors
///
/// `errors.no_last` before any run, or [`run_tool`]'s.
pub fn run_last(host: &mut dyn PluginHost) -> Result<Value, Failure> {
    let last = last::read(host).ok_or_else(|| coded(FailureKind::NotFound, "errors.no_last"))?;
    let args = json!({ "source": last.source.as_str(), "name": last.name, "target": last.views });
    let mut answer = run_tool(host, &args)?;
    answer["$post"] = json!([{ "target": "panel", "message": { "ran": answer.clone() } }]);
    Ok(answer)
}

/// The plugin.
pub struct Snippets;

fn answer(value: Result<Value, Failure>) -> Result<String, Failure> {
    value.map(|value| value.to_string())
}

impl Feature for Snippets {
    fn run_command(
        &mut self,
        host: &mut dyn PluginHost,
        command: &str,
        args: &str,
    ) -> Result<String, Failure> {
        parse(args)?;
        match command {
            PANEL => answer(Ok(json!({ OPEN_KEY: { "panel": PANEL_ID } }))),
            RUN_LAST => answer(run_last(host)),
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
            LIST => answer(list(host, &args)),
            RUN => answer(run_tool(host, &args)),
            SAVE => answer(library::save(host, &args)),
            other => Err(unknown(other)),
        }
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
