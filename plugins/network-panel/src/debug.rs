//! The HTTP debugger's tools (RB-080-37): `debug` (the state for the panel: the held requests,
//! the breakpoints, the composed results, the host's answers), `breakpoint_add`, `_remove`,
//! `_enable`, `breakpoints_toggle` (the command `Ctrl+Shift+Space P R`), and the asks the panel
//! queues for the host - `continue`, `abort`, `compose`, `repeat`. The host runs an ask after
//! the call that queued it (it asks for the rules then) and answers in `$network.answers`;
//! [`crate::rules::document`] drops an answered ask and keeps its error for the panel.

use rb_plugin_api::{Failure, FailureKind, PluginHost};
use serde_json::{Value, json};

use crate::context::{Context, Settings};
use crate::store::{self, MAX_ASKS, MAX_BREAKPOINTS};
use crate::{bad_args, coded};

/// The most times a compose or repeat sends in a row.
pub const MAX_TIMES: u64 = 10;

fn post(message: Value) -> Value {
    json!([{ "target": "panel", "message": message }])
}

/// `debug`: everything the panel's Breakpoints and Compose tabs show.
#[must_use]
pub fn state(host: &mut dyn PluginHost, args: &Value) -> Value {
    let context = Context::read(args);
    let settings = Settings::read(host);
    let state = store::read(host, &context.session);
    json!({
        "held": context.held,
        "breakpoints": state.breakpoints,
        "breakpointsEnabled": state.breakpoints_enabled(),
        "breakpointsActive": context.breakpoints_active,
        "composed": context.composed,
        "answers": context.answers,
        "pending": state.asks.len(),
        "lastError": state.last_error,
        "skipped": context.skipped,
        "timeoutMs": settings.breakpoint_timeout_ms,
        "maxBodyBytes": settings.rewrite_max_body_bytes,
        "rulesRunning": context.present,
    })
}

fn method(raw: &Value) -> Result<Option<String>, Failure> {
    match raw {
        Value::Null => Ok(None),
        Value::String(method) if method.is_empty() => Ok(None),
        Value::String(method)
            if method.len() <= 16 && method.chars().all(|c| c.is_ascii_alphabetic()) =>
        {
            Ok(Some(method.to_ascii_uppercase()))
        }
        _ => Err(bad_args("method")),
    }
}

fn breakpoint(args: &Value, id: String) -> Result<Value, Failure> {
    let pattern = args["urlPattern"]
        .as_str()
        .filter(|pattern| !pattern.is_empty() && pattern.len() <= 2048)
        .ok_or_else(|| bad_args("urlPattern"))?;
    let stage = match args["stage"].as_str() {
        None | Some("request") => "request",
        Some("response") => "response",
        Some(_) => return Err(bad_args("stage")),
    };
    let views = match &args["views"] {
        Value::Null => Value::Null,
        Value::Array(list) if list.len() <= 64 && list.iter().all(Value::is_u64) => {
            Value::Array(list.clone())
        }
        _ => return Err(bad_args("views")),
    };
    let resource_type = match &args["resourceType"] {
        Value::Null => Value::Null,
        Value::String(kind) if kind.is_empty() => Value::Null,
        Value::String(kind) if kind.len() <= 32 => json!(kind),
        _ => return Err(bad_args("resourceType")),
    };
    Ok(json!({
        "id": id,
        "urlPattern": pattern,
        "regex": args["regex"] == Value::Bool(true),
        "method": method(&args["method"])?,
        "resourceType": resource_type,
        "stage": stage,
        "views": views,
        "enabled": args["enabled"] != Value::Bool(false),
    }))
}

fn id_arg(args: &Value) -> Result<String, Failure> {
    args["id"]
        .as_str()
        .filter(|id| !id.is_empty() && id.len() <= 128)
        .map(str::to_owned)
        .ok_or_else(|| bad_args("id"))
}

/// `breakpoint_add`: a breakpoint on `urlPattern` (`regex`, `method`, `resourceType`, `stage`
/// request or response, `views`), with `id` or `bp-<n>`; one with the same id is replaced.
///
/// # Errors
///
/// Arguments that do not read, `plugin.network-panel.errors.too_many` past
/// [`MAX_BREAKPOINTS`], or the state cannot be written.
pub fn breakpoint_add(host: &mut dyn PluginHost, args: &Value) -> Result<Value, Failure> {
    let context = Context::read(args);
    let mut state = store::read(host, &context.session);
    let id = match &args["id"] {
        Value::Null => format!("bp-{}", state.next_seq()),
        _ => id_arg(args)?,
    };
    let breakpoint = breakpoint(args, id.clone())?;
    state
        .breakpoints
        .retain(|known| known["id"] != breakpoint["id"]);
    if state.breakpoints.len() >= MAX_BREAKPOINTS {
        return Err(coded(FailureKind::Limit, "errors.too_many")
            .with_param("limit", MAX_BREAKPOINTS.to_string()));
    }
    state.breakpoints.push(breakpoint.clone());
    store::write(host, &state)?;
    Ok(json!({ "id": id, "breakpoint": breakpoint,
               "$post": post(json!({ "breakpointsChanged": true })) }))
}

/// `breakpoint_remove`.
///
/// # Errors
///
/// `plugin.network-panel.errors.unknown_breakpoint`, or the state cannot be written.
pub fn breakpoint_remove(host: &mut dyn PluginHost, args: &Value) -> Result<Value, Failure> {
    let id = id_arg(args)?;
    let context = Context::read(args);
    let mut state = store::read(host, &context.session);
    let before = state.breakpoints.len();
    state.breakpoints.retain(|known| known["id"] != json!(id));
    if state.breakpoints.len() == before {
        return Err(coded(FailureKind::NotFound, "errors.unknown_breakpoint").with_param("id", &id));
    }
    store::write(host, &state)?;
    Ok(json!({ "id": id, "removed": true, "$post": post(json!({ "breakpointsChanged": true })) }))
}

/// `breakpoint_enable`: one breakpoint on or off.
///
/// # Errors
///
/// As [`breakpoint_remove`], or `enabled` missing.
pub fn breakpoint_enable(host: &mut dyn PluginHost, args: &Value) -> Result<Value, Failure> {
    let id = id_arg(args)?;
    let enabled = args["enabled"]
        .as_bool()
        .ok_or_else(|| bad_args("enabled"))?;
    let context = Context::read(args);
    let mut state = store::read(host, &context.session);
    let known = state
        .breakpoints
        .iter_mut()
        .find(|known| known["id"] == json!(id))
        .ok_or_else(|| {
            coded(FailureKind::NotFound, "errors.unknown_breakpoint").with_param("id", &id)
        })?;
    known["enabled"] = json!(enabled);
    store::write(host, &state)?;
    Ok(
        json!({ "id": id, "enabled": enabled, "$post": post(json!({ "breakpointsChanged": true })) }),
    )
}

/// `breakpoints_toggle`: every breakpoint on or off for this session (`enabled`, else the other
/// way round).
///
/// # Errors
///
/// Arguments that do not read, or the state cannot be written.
pub fn breakpoints_toggle(host: &mut dyn PluginHost, args: &Value) -> Result<Value, Failure> {
    let context = Context::read(args);
    let mut state = store::read(host, &context.session);
    let enabled = match &args["enabled"] {
        Value::Null => !state.breakpoints_enabled(),
        Value::Bool(on) => *on,
        _ => return Err(bad_args("enabled")),
    };
    state.breakpoints_enabled = Some(enabled);
    store::write(host, &state)?;
    Ok(json!({ "breakpointsEnabled": enabled,
               "$post": post(json!({ "breakpointsEnabled": enabled })) }))
}

fn edits(raw: &Value) -> Result<Value, Failure> {
    match raw {
        Value::Null => Ok(json!({})),
        Value::Object(_) => Ok(raw.clone()),
        _ => Err(bad_args("edits")),
    }
}

fn times(raw: &Value) -> Result<u64, Failure> {
    match raw {
        Value::Null => Ok(1),
        value => value
            .as_u64()
            .filter(|times| (1..=MAX_TIMES).contains(times))
            .ok_or_else(|| bad_args("times")),
    }
}

/// Queues `ask` for the host and answers its id.
fn queue(host: &mut dyn PluginHost, args: &Value, mut ask: Value) -> Result<Value, Failure> {
    let context = Context::read(args);
    let mut state = store::read(host, &context.session);
    if state.asks.len() >= MAX_ASKS {
        return Err(
            coded(FailureKind::Limit, "errors.too_many").with_param("limit", MAX_ASKS.to_string())
        );
    }
    let id = format!("a{}", state.next_seq());
    ask["id"] = json!(id);
    state.asks.push(ask);
    state.last_error = None;
    store::write(host, &state)?;
    Ok(json!({ "ask": id, "queued": true }))
}

fn held_ask(args: &Value, kind: &str) -> Result<Value, Failure> {
    let held = id_arg(args)?;
    let mut ask =
        json!({ "kind": kind, "held": held, "allViews": args["allViews"] == Value::Bool(true) });
    if kind == "continue" {
        ask["edits"] = edits(&args["edits"])?;
    }
    Ok(ask)
}

/// `continue`: lets held request `id` go on, with `edits` (method, url, headers, body, status),
/// and with `allViews` the same request held in every view.
///
/// # Errors
///
/// Arguments that do not read, or the state cannot be written.
pub fn continue_held(host: &mut dyn PluginHost, args: &Value) -> Result<Value, Failure> {
    queue(host, args, held_ask(args, "continue")?)
}

/// `abort`: fails held request `id`, with `allViews` the same request in every view.
///
/// # Errors
///
/// As [`continue_held`].
pub fn abort(host: &mut dyn PluginHost, args: &Value) -> Result<Value, Failure> {
    queue(host, args, held_ask(args, "abort")?)
}

/// `compose`: sends `request` (`method`, `url`, `headers`, `body`) from `view`'s context,
/// `times` in a row; the results arrive in `debug`'s `composed`.
///
/// # Errors
///
/// As [`continue_held`].
pub fn compose(host: &mut dyn PluginHost, args: &Value) -> Result<Value, Failure> {
    let request = &args["request"];
    if !request.is_object() || !request["url"].is_string() {
        return Err(bad_args("request"));
    }
    let view = match &args["view"] {
        Value::Null => Value::Null,
        value if value.is_u64() => value.clone(),
        _ => return Err(bad_args("view")),
    };
    queue(
        host,
        args,
        json!({ "kind": "compose", "view": view, "request": request, "times": times(&args["times"])? }),
    )
}

/// `repeat`: sends captured request `requestId` of `view` again with `edits`, `times` in a row.
///
/// # Errors
///
/// As [`continue_held`].
pub fn repeat(host: &mut dyn PluginHost, args: &Value) -> Result<Value, Failure> {
    let view = args["view"].as_u64().ok_or_else(|| bad_args("view"))?;
    let request_id = args["requestId"]
        .as_str()
        .filter(|id| !id.is_empty() && id.len() <= 200)
        .ok_or_else(|| bad_args("requestId"))?;
    queue(
        host,
        args,
        json!({ "kind": "repeat", "view": view, "requestId": request_id,
                "edits": edits(&args["edits"])?, "times": times(&args["times"])? }),
    )
}

#[cfg(test)]
#[path = "debug_tests.rs"]
mod tests;
