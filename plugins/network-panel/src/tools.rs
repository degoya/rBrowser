//! The list, the body preview, the session's switches and the HAR tools.

use std::collections::BTreeSet;

use rb_plugin_api::{Failure, FailureKind, PluginHost, ViewId};
use serde_json::{Value, json};

use crate::context::{Context, Settings};
use crate::requests::{self, Filter};
use crate::{bad_args, coded, har, har_import, store};

/// The rows of `list` by default and at most.
pub const LIMIT: (usize, usize) = (150, 500);
/// The JSON bytes of the rows of one answer: a panel takes 64 KiB in all.
pub const ROWS_BUDGET: usize = 48 * 1024;
/// The most body text the preview answers.
pub const BODY_PREVIEW: usize = 32 * 1024;

fn filter(args: &Value) -> Result<Filter, Failure> {
    let search = match &args["search"] {
        Value::Null => None,
        Value::String(text) if text.trim().is_empty() => None,
        Value::String(text) => Some(
            text.trim()
                .chars()
                .take(200)
                .collect::<String>()
                .to_lowercase(),
        ),
        _ => return Err(bad_args("search")),
    };
    let types = match &args["types"] {
        Value::Null => BTreeSet::new(),
        Value::Array(types) => types
            .iter()
            .map(|kind| kind.as_str().map(str::to_owned))
            .collect::<Option<_>>()
            .ok_or_else(|| bad_args("types"))?,
        _ => return Err(bad_args("types")),
    };
    let status = match args["status"].as_str() {
        None | Some("all") => None,
        Some(status @ ("ok" | "error" | "failed")) => Some(status.to_owned()),
        Some(_) => return Err(bad_args("status")),
    };
    Ok(Filter {
        search,
        types,
        status,
        differences: args["differences"] == Value::Bool(true),
    })
}

/// The views with their engine and whether the host intercepts them.
fn views(host: &mut dyn PluginHost, context: &Context) -> Vec<Value> {
    host.views()
        .unwrap_or_default()
        .into_iter()
        .map(|view| {
            let state = context.views.iter().find(|state| state.view == view.id);
            json!({ "id": view.id, "label": view.label, "breakpoint": view.breakpoint,
                    "engine": view.engine, "width": view.width,
                    "supported": state.is_some_and(|s| s.supported),
                    "intercepting": state.is_some_and(|s| s.intercepting) })
        })
        .collect()
}

/// `list`: the newest rows the filter passes, the views, and the state of the rules.
///
/// # Errors
///
/// Arguments that do not read.
pub fn list(host: &mut dyn PluginHost, args: &Value) -> Result<Value, Failure> {
    let filter = filter(args)?;
    let limit = match &args["limit"] {
        Value::Null => LIMIT.0,
        value => value
            .as_u64()
            .and_then(|n| usize::try_from(n).ok())
            .filter(|n| (1..=LIMIT.1).contains(n))
            .ok_or_else(|| bad_args("limit"))?,
    };
    let context = Context::read(args);
    let settings = Settings::read(host);
    let state = store::read(host, &context.session);
    let capture = requests::capture(args);
    let (rows, total) = requests::page(&capture, &filter, limit, ROWS_BUDGET);
    Ok(json!({
        "rows": rows,
        "total": total,
        "captured": capture.captured,
        "views": views(host, &context),
        "trusted": context.trusted,
        "rulesRunning": context.present,
        "applied": context.applied,
        "mocksEnabled": state.mocks_enabled(&settings),
        "cacheDisabled": state.cache_disabled(&settings),
    }))
}

/// `body`: the response body of request `id` in `view` while the view still holds it, cut.
///
/// # Errors
///
/// `plugin.network-panel.errors.no_body`, or arguments that do not read.
pub fn body(host: &mut dyn PluginHost, args: &Value) -> Result<Value, Failure> {
    let view = args["view"]
        .as_u64()
        .and_then(|view| ViewId::try_from(view).ok())
        .ok_or_else(|| bad_args("view"))?;
    let id = args["id"]
        .as_str()
        .filter(|id| !id.is_empty() && id.len() <= 200)
        .ok_or_else(|| bad_args("id"))?;
    let params = json!({ "requestId": id }).to_string();
    let answer = host
        .cdp_send(view, "Network.getResponseBody", &params)
        .map_err(|_| coded(FailureKind::NotFound, "errors.no_body"))?;
    let answer: Value = serde_json::from_str(&answer).unwrap_or(Value::Null);
    let text = answer["body"]
        .as_str()
        .ok_or_else(|| coded(FailureKind::NotFound, "errors.no_body"))?;
    let shown: String = text.chars().take(BODY_PREVIEW).collect();
    Ok(json!({
        "text": shown,
        "base64": answer["base64Encoded"] == true,
        "truncated": shown.len() < text.len(),
    }))
}

fn switch(args: &Value, key: &str, current: bool) -> Result<bool, Failure> {
    match &args[key] {
        Value::Null => Ok(!current),
        Value::Bool(on) => Ok(*on),
        _ => Err(bad_args(key)),
    }
}

/// `mocks_toggle`: mocks on or off for this session (`enabled`, else the other way round).
///
/// # Errors
///
/// Arguments that do not read, or the state cannot be written.
pub fn mocks_toggle(host: &mut dyn PluginHost, args: &Value) -> Result<Value, Failure> {
    let context = Context::read(args);
    let settings = Settings::read(host);
    let mut state = store::read(host, &context.session);
    let enabled = switch(args, "enabled", state.mocks_enabled(&settings))?;
    state.mocks_enabled = Some(enabled);
    store::write(host, &state)?;
    Ok(json!({ "mocksEnabled": enabled,
               "$post": [{ "target": "panel", "message": { "mocksEnabled": enabled } }] }))
}

/// `cache_toggle`: the cache off or on for this session (`disabled`, else the other way round).
///
/// # Errors
///
/// As [`mocks_toggle`].
pub fn cache_toggle(host: &mut dyn PluginHost, args: &Value) -> Result<Value, Failure> {
    let context = Context::read(args);
    let settings = Settings::read(host);
    let mut state = store::read(host, &context.session);
    let disabled = switch(args, "disabled", state.cache_disabled(&settings))?;
    state.cache_disabled = Some(disabled);
    store::write(host, &state)?;
    Ok(json!({ "cacheDisabled": disabled,
               "$post": [{ "target": "panel", "message": { "cacheDisabled": disabled } }] }))
}

/// The command `site_data_clear`: with `confirmed: true` (the panel's confirmation) the host
/// clears every view's site data once; without, the panel is asked to confirm.
///
/// # Errors
///
/// The state cannot be written.
pub fn site_data_clear(host: &mut dyn PluginHost, args: &Value) -> Result<Value, Failure> {
    if args["confirmed"] != Value::Bool(true) {
        return Ok(json!({
            "confirm": true,
            crate::OPEN_KEY: { "panel": crate::PANEL_ID },
            "$post": [{ "target": "panel", "message": { "confirmClear": true } }],
        }));
    }
    let context = Context::read(args);
    let mut state = store::read(host, &context.session);
    state.clear_site_data += 1;
    store::write(host, &state)?;
    Ok(json!({ "cleared": true,
               "$post": [{ "target": "panel", "message": { "cleared": true } }] }))
}

/// `har_export`: MCP's `target` a view id exports that view; else `merged` (default) or one
/// file per view; `bodies` defaults to the setting `harBodies`.
///
/// # Errors
///
/// Arguments that do not read, or a file cannot be written.
pub fn har_export(host: &mut dyn PluginHost, args: &Value) -> Result<Value, Failure> {
    let settings = Settings::read(host);
    let only = args["target"]
        .as_u64()
        .and_then(|view| ViewId::try_from(view).ok());
    let flag = |key: &str, default: bool| match &args[key] {
        Value::Null => Ok(default),
        Value::Bool(on) => Ok(*on),
        _ => Err(bad_args(key)),
    };
    let merged = flag("merged", true)?;
    let bodies = flag("bodies", settings.har_bodies)?;
    har::export(host, args, only, merged, bodies)
}

/// `har_import`: under workspace trust only (the rules it makes are mocks).
///
/// # Errors
///
/// `plugin.network-panel.errors.untrusted`, or [`har_import::import`]'s.
pub fn har_import(host: &mut dyn PluginHost, args: &Value) -> Result<Value, Failure> {
    if !Context::read(args).trusted {
        return Err(coded(FailureKind::AccessDenied, "errors.untrusted"));
    }
    let path = args["path"].as_str().ok_or_else(|| bad_args("path"))?;
    har_import::import(host, path)
}

#[cfg(test)]
#[path = "tools_tests.rs"]
mod tests;
