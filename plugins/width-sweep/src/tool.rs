//! The command and tool `run` (`plugin.width-sweep.run`, MCP `sweep_run`) and `cancel`
//! (`plugin.width-sweep.cancel`, MCP `sweep_cancel`).
//!
//! `run` sweeps the view `target` names (the context menu's clicked view; the active view
//! without one or with `"all"`) from `from` to `to` in steps of `step` for the `kinds` (each the
//! settings' value when absent), reports the ranges found to the findings panel - an empty run
//! too, so the panel shows the clean result - and answers what it saw. A sweep ended by Escape or
//! a cancel reports nothing: half a sweep cannot tell where a range ends.
//!
//! `cancel` ends the plugin's width lease of every Chromium view
//! (`Emulation.clearDeviceMetricsOverride`); a sweep running beside it stops at its next width.

use rb_plugin_api::{Failure, FailureKind, PluginHost, ViewInfo, codes};
use serde_json::{Value, json};

use crate::findings;
use crate::plan;
use crate::probe::CLEAR;
use crate::settings::Settings;
use crate::sweep::{self, End, Request};

/// The most ranges the answer lists; the findings panel has them all.
pub const MAX_LISTED: usize = 50;

fn invalid(argument: &str) -> Failure {
    Failure::coded(FailureKind::BadData, codes::INVALID).with_param("argument", argument)
}

/// A call's arguments: a JSON object (empty text is `{}`).
///
/// # Errors
///
/// `plugins.error.invalid` for anything else.
pub fn parse(args: &str) -> Result<Value, Failure> {
    if args.trim().is_empty() {
        return Ok(json!({}));
    }
    match serde_json::from_str::<Value>(args) {
        Ok(value) if value.is_object() => Ok(value),
        _ => Err(invalid("arguments")),
    }
}

/// The view `target` names: a view id, else (none, `"all"`) the active view, else the first.
///
/// # Errors
///
/// A `target` of another kind, or no such view.
pub fn target(host: &mut dyn PluginHost, target: Option<&Value>) -> Result<ViewInfo, Failure> {
    let views = host.views()?;
    let named = match target {
        None | Some(Value::Null) => None,
        Some(Value::String(all)) if all == "all" => None,
        Some(Value::Number(number)) => Some(
            number
                .as_u64()
                .and_then(|id| u32::try_from(id).ok())
                .ok_or_else(|| invalid("target"))?,
        ),
        Some(_) => return Err(invalid("target")),
    };
    let id = match named {
        Some(id) => Some(id),
        None => host.active_view()?,
    };
    let not_found = |view: String| {
        Failure::coded(FailureKind::NotFound, codes::NOT_FOUND).with_param("view", view)
    };
    match id {
        Some(id) => views
            .into_iter()
            .find(|view| view.id == id)
            .ok_or_else(|| not_found(id.to_string())),
        None => views
            .into_iter()
            .next()
            .ok_or_else(|| not_found("none".to_owned())),
    }
}

fn end_name(end: End) -> &'static str {
    match end {
        End::Done => "done",
        End::Cancelled => "cancelled",
        End::Escaped => "escaped",
    }
}

/// `run`: one sweep; the answer names the view, the widths, how it ended, how many widths were
/// looked at and - when it ran through - the ranges found.
///
/// # Errors
///
/// A bad argument, a view that is gone or runs another engine, a width the host refused, the
/// page's failure, or the findings not taken.
pub fn run(host: &mut dyn PluginHost, args: &Value) -> Result<Value, Failure> {
    let view = target(host, args.get("target"))?;
    let settings = Settings::read(host).with_args(args)?;
    let span = settings.span()?;
    // Before the first width: a Firefox or WebKit view is never leased (RB-080-98).
    view.require_chromium("cdp")?;
    let request = Request {
        view: view.id,
        span,
        kinds: settings.kinds.clone(),
    };
    let outcome = sweep::sweep(host, &request)?;
    let mut answer = json!({
        "view": view.id,
        "from": span.from,
        "to": span.to,
        "step": span.step,
        "kinds": settings.kinds,
        "end": end_name(outcome.end),
        "probes": outcome.probes,
        "mismatched": outcome.mismatched,
    });
    if outcome.end != End::Done {
        return Ok(answer);
    }
    let ranges = plan::ranges(&outcome.samples);
    let found = findings::findings(&ranges, &view);
    host.report_findings(&found)?;
    let mut listed: Vec<&plan::Range> = ranges.iter().collect();
    listed.sort_by(|a, b| (a.from, a.to, &a.issue).cmp(&(b.from, b.to, &b.issue)));
    let listed: Vec<Value> = listed
        .into_iter()
        .take(MAX_LISTED)
        .map(|range| {
            json!({
                "kind": range.issue.kind,
                "from": range.from,
                "to": range.to,
                "selector": range.issue.selector,
                "other": range.issue.other,
            })
        })
        .collect();
    if let Some(object) = answer.as_object_mut() {
        object.insert("count".to_owned(), json!(found.len()));
        object.insert("findings".to_owned(), Value::Array(listed));
    }
    Ok(answer)
}

/// `cancel`: ends the lease of every Chromium view; `{ "cancelled": true }` when one was live.
///
/// # Errors
///
/// The views cannot be listed.
pub fn cancel(host: &mut dyn PluginHost) -> Result<Value, Failure> {
    let mut cancelled = false;
    for view in host.views()? {
        if view.require_chromium("cdp").is_err() {
            continue;
        }
        let ended = host
            .cdp_send(view.id, CLEAR, "{}")
            .ok()
            .and_then(|answer| serde_json::from_str::<Value>(&answer).ok())
            .and_then(|answer| answer.get("ended").and_then(Value::as_bool))
            .unwrap_or(false);
        cancelled |= ended;
    }
    Ok(json!({ "cancelled": cancelled }))
}

#[cfg(test)]
#[path = "tool_tests.rs"]
mod tests;
