//! The MCP tools and the command: `run` (`lighthouse_run`, the command `plugin.lighthouse.run`),
//! `status` (`lighthouse_status`) and `cancel` (`lighthouse_cancel`). Arguments are read
//! strictly: what the manifest's input schema allows plus the host's `target` (`"all"` or a
//! view id) and `$scope` (RB-099-10: `"all"`, `"visible"` or breakpoint ids - the host resolved it
//! into `breakpoints`; the run's summary keeps it, so the panel shows the last choice again).
//! `status` names the breakpoints the views show, the panel's choice of breakpoints.

use rb_plugin_api::{Failure, PluginHost};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::run::{self, LATEST, Request, out_path};
use crate::{bad_args, rules, sidecar};

/// The most breakpoints a call names.
pub const MAX_BREAKPOINTS: usize = 32;
const MAX_URL: usize = 8 * 1024;

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct RunArgs {
    breakpoints: Option<Vec<String>>,
    categories: Option<Vec<String>>,
    url: Option<String>,
    target: Option<Value>,
    #[serde(rename = "$scope")]
    scope: Option<Value>,
}

/// Whether `scope` reads as a run's scope: `"all"`, `"visible"` or breakpoint ids.
fn is_scope(scope: &Value) -> bool {
    match scope {
        Value::String(word) => word == "all" || word == "visible",
        Value::Array(ids) => {
            ids.len() <= MAX_BREAKPOINTS
                && ids.iter().all(|id| {
                    id.as_str()
                        .is_some_and(|id| !id.is_empty() && id.len() <= 64)
                })
        }
        _ => false,
    }
}

/// Reads the arguments of `run`.
///
/// # Errors
///
/// They do not read or leave their bounds (`errors.bad_args`).
pub fn run_request(args: &str) -> Result<Request, Failure> {
    let parsed: RunArgs = if args.trim().is_empty() {
        RunArgs::default()
    } else {
        serde_json::from_str(args).map_err(|error| bad_args(&error.to_string()))?
    };
    if let Some(list) = &parsed.breakpoints
        && (list.len() > MAX_BREAKPOINTS || list.iter().any(|b| b.is_empty() || b.len() > 64))
    {
        return Err(bad_args(
            "breakpoints: at most 32 ids of 1 to 64 characters",
        ));
    }
    if let Some(list) = &parsed.categories
        && let Some(bad) = list.iter().find(|c| !rules::is_category(c))
    {
        return Err(bad_args(&format!(
            "categories: `{bad}` is no Lighthouse category"
        )));
    }
    if let Some(url) = &parsed.url
        && (url.len() > MAX_URL || !crate::plan::is_web_url(url))
    {
        return Err(bad_args("url: an http or https URL"));
    }
    if parsed.scope.as_ref().is_some_and(|scope| !is_scope(scope)) {
        return Err(bad_args(
            "$scope: \"all\", \"visible\" or a list of breakpoint ids",
        ));
    }
    let view = match &parsed.target {
        None | Some(Value::Null) => None,
        Some(Value::String(all)) if all == "all" => None,
        Some(Value::Number(id)) => Some(
            id.as_u64()
                .and_then(|id| u32::try_from(id).ok())
                .ok_or_else(|| bad_args("target: a view id"))?,
        ),
        Some(_) => return Err(bad_args("target: \"all\" or a view id")),
    };
    Ok(Request {
        breakpoints: parsed.breakpoints,
        categories: parsed.categories,
        url: parsed.url,
        view,
        report: false,
        scope: parsed.scope,
    })
}

/// `run`: the command reports its findings, the MCP tool only answers them. The answer asks the
/// host to remove the runs past `lighthouse.keepRuns` (RB-091-02, [`crate::reports::retain`]).
///
/// # Errors
///
/// [`run_request`]'s and [`run::run`]'s.
pub fn run(host: &mut dyn PluginHost, args: &str, report: bool) -> Result<String, Failure> {
    let mut request = run_request(args)?;
    request.report = report;
    let mut summary = run::run(host, &request)?;
    crate::reports::retain(host, &mut summary);
    Ok(summary.to_string())
}

/// `status`: the sidecar's runs and the last run's summary (`null` before the first).
///
/// # Errors
///
/// None: an unavailable sidecar is part of the answer.
pub fn status(host: &mut dyn PluginHost) -> Result<String, Failure> {
    let sidecar = match sidecar::status(host) {
        Ok(state) => {
            json!({ "available": true, "running": state["running"], "queued": state["queued"] })
        }
        Err(failure) => json!({
            "available": false,
            "error": failure.code(),
            "reason": failure.param("reason"),
        }),
    };
    let latest = host
        .project_read(&out_path(LATEST))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
        .unwrap_or(Value::Null);
    let breakpoints = shown_breakpoints(host);
    Ok(json!({ "sidecar": sidecar, "latest": latest, "breakpoints": breakpoints }).to_string())
}

/// The breakpoints the views show, once each in grid order, with their engine (RB-099-10).
fn shown_breakpoints(host: &mut dyn PluginHost) -> Vec<Value> {
    let mut shown: Vec<Value> = Vec::new();
    for view in host.views().unwrap_or_default() {
        let seen = shown
            .iter()
            .any(|entry| entry["id"].as_str() == Some(view.breakpoint.as_str()));
        if !view.breakpoint.is_empty() && !seen {
            shown
                .push(json!({ "id": view.breakpoint, "label": view.label, "engine": view.engine }));
        }
    }
    shown
}

/// `cancel`: ends the running Lighthouse run and the queued ones.
///
/// # Errors
///
/// The sidecar is not there.
pub fn cancel(host: &mut dyn PluginHost) -> Result<String, Failure> {
    Ok(json!({ "cancelled": sidecar::cancel(host)? }).to_string())
}

#[cfg(test)]
#[path = "tool_tests.rs"]
mod tests;
