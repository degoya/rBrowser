//! The commands, the MCP tools and the page script's requests.
//!
//! - `overlay` (`containers_overlay`, command `overlay`): `{ on?, target? }` shows or hides the
//!   container overlay in every view, one, or the active one (`"active"`); without `on` it
//!   toggles. Answers `{ on, target }` and posts `{ type: "state", on, settings }` to the page
//!   scripts of the target and `{ type: "overlay", on, target }` to the panel.
//! - `analyze` (`containers_analyze`): `{ target? }` evaluates the analysis in each view of the
//!   target and answers the merged [`analysis::Analysis`] with `overlay`, whether the overlay
//!   shows in the target. The command `analyze` opens the panel (`$open`) and asks an open one
//!   to analyze again (`{ type: "refresh" }`).
//! - `resize` (`containers_resize`): `{ path, width, target? }` pins the width of the container
//!   at `path` in each view of the target, `{ reset: true, path?, target? }` brings back the
//!   inline style of that container or of every container rBrowser resized. Answers
//!   `{ views: [{ view, resized | reset } | { view, error }] }` and asks the panel to refresh.
//! - `@page`: `{ type: "hello" }` answers `{ on, settings }` for the view.

use rb_plugin_api::{Failure, FailureKind, PluginHost, ViewId, ViewInfo, codes};
use serde_json::{Map, Value, json};

use crate::analysis::{self, ViewReport};
use crate::script;
use crate::settings;
use crate::state::{State, Target};

/// The longest container path `resize` takes, in characters.
pub const MAX_PATH_CHARS: usize = 300;
/// The widest width `resize` sets, in px (`web/src/resize.ts`, `MAX_WIDTH_PX`).
pub const MAX_WIDTH_PX: f64 = 100_000.0;

pub(crate) fn bad_args() -> Failure {
    Failure::coded(FailureKind::BadData, codes::INVALID).with_param("reason", "arguments")
}

/// The arguments: a JSON object.
fn object(args: &str) -> Result<Map<String, Value>, Failure> {
    match serde_json::from_str(args) {
        Ok(Value::Object(map)) => Ok(map),
        _ => Err(bad_args()),
    }
}

/// The target of a call: a view id, the active view for `"active"`, else every view.
fn target_of(host: &mut dyn PluginHost, args: &Map<String, Value>) -> Target {
    if args.get("target").and_then(Value::as_str) == Some("active")
        && let Ok(Some(view)) = host.active_view()
    {
        return Target::View(view);
    }
    Target::from_args(&Value::Object(args.clone()))
}

/// The views of `target`, with their labels and widths where the host lists them.
fn target_views(host: &mut dyn PluginHost, target: Target) -> Result<Vec<ViewInfo>, Failure> {
    match target {
        Target::All => host.views(),
        Target::View(id) => {
            let known = host
                .views()
                .ok()
                .and_then(|views| views.into_iter().find(|view| view.id == id));
            Ok(vec![known.unwrap_or(ViewInfo {
                id,
                ..ViewInfo::default()
            })])
        }
    }
}

/// Where a post for `target` goes: `"all"` or the view id.
fn post_target(target: Target) -> Value {
    match target {
        Target::All => json!("all"),
        Target::View(view) => json!(view),
    }
}

/// The panel's cue to analyze again.
fn refresh_post() -> Value {
    json!({ "target": "panel", "message": { "type": "refresh" } })
}

/// Saves `state`; a project without an out folder keeps the change for the posts only.
fn store(host: &mut dyn PluginHost, state: &State) {
    if let Err(failure) = state.save(host) {
        let code = failure.code().unwrap_or("io").to_owned();
        host.log(
            "warn",
            &format!("container-queries: the overlay state was not saved ({code})"),
        );
    }
}

/// `overlay`, the tool and the command.
///
/// # Errors
///
/// Arguments that are no object, or an `on` that is no boolean.
pub fn overlay(host: &mut dyn PluginHost, args: &str) -> Result<String, Failure> {
    let args = object(args)?;
    let target = target_of(host, &args);
    let mut state = State::load(host);
    let on = match args.get("on") {
        None | Some(Value::Null) => !state.effective(target.view()),
        Some(value) => value.as_bool().ok_or_else(bad_args)?,
    };
    state.set(target, on);
    store(host, &state);
    let to = post_target(target);
    let message = json!({
        "type": "state",
        "on": on,
        "settings": settings::page_settings(host),
    });
    let answer = json!({
        "on": on,
        "target": to,
        "$post": [
            { "target": to, "message": message },
            { "target": "panel", "message": { "type": "overlay", "on": on, "target": to } },
        ],
    });
    Ok(answer.to_string())
}

/// `analyze`, the tool.
///
/// # Errors
///
/// Arguments that are no object; the views cannot be listed for every view.
pub fn analyze(host: &mut dyn PluginHost, args: &str) -> Result<String, Failure> {
    let args = object(args)?;
    let target = target_of(host, &args);
    let script = script::analyze_script();
    let mut results = Vec::new();
    for view in target_views(host, target)? {
        let result = match host.evaluate(view.id, &script) {
            Ok(answer) => ViewReport::parse(&answer).ok_or_else(|| codes::BAD_OUTPUT.to_owned()),
            Err(failure) => Err(failure.code().unwrap_or(codes::HOST_FAILED).to_owned()),
        };
        results.push((view, result));
    }
    let mut answer = serde_json::to_value(analysis::merge(results))
        .map_err(|_| Failure::coded(FailureKind::Other, codes::BAD_OUTPUT))?;
    let overlay = State::load(host).effective(target.view());
    if let Some(fields) = answer.as_object_mut() {
        fields.insert("overlay".to_owned(), Value::Bool(overlay));
    }
    Ok(answer.to_string())
}

/// The command `analyze`: the panel opened, and asked to analyze again when it is open already.
#[must_use]
pub fn analyze_command() -> String {
    json!({ "$open": { "panel": "main" }, "$post": [refresh_post()] }).to_string()
}

/// `resize`, the tool.
///
/// # Errors
///
/// Arguments that are no object; a `path` that is no text of 1 to 300 characters; no `path` or
/// no `width` from 0 to 100000 without `reset`; the views cannot be listed for every view.
pub fn resize(host: &mut dyn PluginHost, args: &str) -> Result<String, Failure> {
    let args = object(args)?;
    let target = target_of(host, &args);
    let reset = match args.get("reset") {
        None | Some(Value::Null) => false,
        Some(value) => value.as_bool().ok_or_else(bad_args)?,
    };
    let path = match args.get("path") {
        None | Some(Value::Null) => None,
        Some(Value::String(path)) if !path.is_empty() && path.chars().count() <= MAX_PATH_CHARS => {
            Some(path.clone())
        }
        Some(_) => return Err(bad_args()),
    };
    let script = if reset {
        script::reset_script(path.as_deref())
    } else {
        let path = path.ok_or_else(bad_args)?;
        let width = args
            .get("width")
            .and_then(Value::as_f64)
            .filter(|width| width.is_finite() && (0.0..=MAX_WIDTH_PX).contains(width))
            .ok_or_else(bad_args)?;
        script::resize_script(&path, width)
    };
    let key = if reset { "reset" } else { "resized" };
    let mut entries = Vec::new();
    for view in target_views(host, target)? {
        let mut entry = Map::new();
        entry.insert("view".to_owned(), json!(view.id));
        match host.evaluate(view.id, &script) {
            Ok(answer) => match serde_json::from_str::<Value>(answer.trim()) {
                Ok(value @ (Value::Bool(_) | Value::Number(_))) => {
                    entry.insert(key.to_owned(), value);
                }
                _ => {
                    entry.insert("error".to_owned(), json!(codes::BAD_OUTPUT));
                }
            },
            Err(failure) => {
                let code = failure.code().unwrap_or(codes::HOST_FAILED).to_owned();
                entry.insert("error".to_owned(), json!(code));
            }
        }
        entries.push(Value::Object(entry));
    }
    Ok(json!({ "views": entries, "$post": [refresh_post()] }).to_string())
}

/// A page script's request: `{ view, message }`.
///
/// # Errors
///
/// Not the host's `@page` shape, or a message of an unknown type.
pub fn page(host: &mut dyn PluginHost, args: &str) -> Result<String, Failure> {
    let args = object(args)?;
    let view: ViewId = args
        .get("view")
        .and_then(Value::as_u64)
        .and_then(|view| ViewId::try_from(view).ok())
        .ok_or_else(bad_args)?;
    let message = args.get("message").ok_or_else(bad_args)?;
    match message.get("type").and_then(Value::as_str) {
        Some("hello") => {
            let on = State::load(host).effective(Some(view));
            let answer = json!({ "on": on, "settings": settings::page_settings(host) });
            Ok(answer.to_string())
        }
        _ => Err(bad_args()),
    }
}

/// At a start the state of single views is gone; whether every view shows the overlay stays.
pub fn activate(host: &mut dyn PluginHost) {
    let state = State::load(host);
    let mut next = state.clone();
    next.forget_views();
    if next != state {
        store(host, &next);
    }
}
