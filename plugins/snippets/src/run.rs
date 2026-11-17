//! Running a snippet in views (RB-070-04): one evaluation per view over the plugin's `cdp`
//! grant, results joined - equal outcomes of several views are one result naming its views, as
//! the console's command line does.
//!
//! - **`main`**: `Runtime.evaluate` without a context, the page's own world of the main frame.
//! - **`isolated`**: the plugin's isolated world `rbplugin-snippets` of the main frame
//!   (`Page.getFrameTree`, `Page.createIsolatedWorld`; Chromium keeps one world per name and
//!   frame), then `Runtime.evaluate` in it - the DOM is the page's, its globals are not.
//!
//! Both with `returnByValue`, `awaitPromise` and the snippet's own `timeout`, which cuts an
//! endless synchronous loop; the wrapper ([`crate::script`]) races the promise against the same
//! timeout. The views are run one after the other; a run that would outlast the component's
//! own time ([`BUDGET_SECONDS`]) leaves the remaining views with `errors.budget`. CDP is
//! Chromium's: a Firefox or WebKit view answers `plugins.error.not_available`.

use rb_plugin_api::{Failure, FailureKind, PluginHost, ViewId, ViewInfo};
use serde_json::{Value, json};

use crate::header::World;
use crate::script;
use crate::text::cut;

/// The isolated world snippets run in: the plugin's own (`rbplugin-<id>`).
pub const WORLD_NAME: &str = "rbplugin-snippets";
/// How long one run may take in all, seconds: below three times the manifest's
/// `limits.timeout_milliseconds` (30 s), the component's own wall clock.
pub const BUDGET_SECONDS: u64 = 80;

/// Where a snippet runs by default (header `@target`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RunTarget {
    #[default]
    All,
    Active,
}

impl RunTarget {
    /// `all` or `active`.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "all" => Some(Self::All),
            "active" => Some(Self::Active),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Active => "active",
        }
    }
}

/// The views of one run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    All,
    Active,
    View(ViewId),
    Views(Vec<ViewId>),
}

impl Target {
    /// The target `value` names: `"all"`, `"active"`, a view id or a list of them; `default`
    /// for none.
    ///
    /// # Errors
    ///
    /// Another value.
    pub fn parse(value: &Value, default: RunTarget) -> Result<Self, Failure> {
        let id = |value: &Value| value.as_u64().and_then(|id| ViewId::try_from(id).ok());
        match value {
            Value::Null => Ok(match default {
                RunTarget::All => Self::All,
                RunTarget::Active => Self::Active,
            }),
            Value::String(name) if name == "all" => Ok(Self::All),
            Value::String(name) if name == "active" => Ok(Self::Active),
            Value::Number(_) => id(value)
                .map(Self::View)
                .ok_or_else(|| crate::bad_args("target")),
            Value::Array(ids) if !ids.is_empty() && ids.len() <= 64 => ids
                .iter()
                .map(id)
                .collect::<Option<Vec<_>>>()
                .map(Self::Views)
                .ok_or_else(|| crate::bad_args("target")),
            _ => Err(crate::bad_args("target")),
        }
    }
}

/// The views of `target` among `known`, in grid order for `All`; a named view that is gone stays
/// in the list (its result says so).
///
/// # Errors
///
/// `errors.no_active_view` for `Active` without one.
pub fn resolve(
    host: &mut dyn PluginHost,
    known: &[ViewInfo],
    target: &Target,
) -> Result<Vec<ViewId>, Failure> {
    Ok(match target {
        Target::All => known.iter().map(|view| view.id).collect(),
        Target::Active => vec![
            host.active_view()?
                .ok_or_else(|| crate::coded(FailureKind::NotFound, "errors.no_active_view"))?,
        ],
        Target::View(view) => vec![*view],
        Target::Views(views) => {
            let mut unique = Vec::new();
            for view in views {
                if !unique.contains(view) {
                    unique.push(*view);
                }
            }
            unique
        }
    })
}

/// What one snippet run needs.
#[derive(Clone, Copy, Debug)]
pub struct Job<'a> {
    pub code: &'a str,
    pub world: World,
    pub timeout_ms: u32,
}

/// One view's outcome: the wrapper's object, or a failure.
fn outcome_of(answer: &str) -> Value {
    let Ok(answer) = serde_json::from_str::<Value>(answer) else {
        return failure_outcome("plugins.error.bad_output", false);
    };
    if let Some(details) = answer.get("exceptionDetails") {
        let text = details["exception"]["description"]
            .as_str()
            .or_else(|| details["text"].as_str())
            .unwrap_or("Error");
        let timed_out = text.contains("Execution was terminated");
        return json!({ "ok": false, "type": "error", "text": cut(text, script::MAX_TEXT_CHARS),
                       "timedOut": timed_out });
    }
    let value = &answer["result"]["value"];
    match value.get("ok") {
        Some(Value::Bool(_)) => value.clone(),
        _ => json!({ "ok": true, "type": "unknown",
                     "text": cut(&value.to_string(), script::MAX_TEXT_CHARS) }),
    }
}

fn failure_outcome(code: &str, timed_out: bool) -> Value {
    json!({ "ok": false, "type": "failure", "text": code, "timedOut": timed_out })
}

fn from_failure(failure: &Failure) -> Value {
    let timed_out = failure
        .params
        .get("reason")
        .is_some_and(|reason| reason == "timeout" || reason.contains("terminated"));
    let code = failure
        .code
        .clone()
        .unwrap_or_else(|| "plugins.error.host_failed".to_owned());
    failure_outcome(&code, timed_out)
}

/// The execution context of the isolated world in `view`'s main frame.
fn isolated_context(host: &mut dyn PluginHost, view: ViewId) -> Result<i64, Failure> {
    let tree: Value = serde_json::from_str(&host.cdp_send(view, "Page.getFrameTree", "{}")?)
        .map_err(|_| crate::coded(FailureKind::Other, "errors.frame"))?;
    let frame = tree["frameTree"]["frame"]["id"]
        .as_str()
        .ok_or_else(|| crate::coded(FailureKind::Other, "errors.frame"))?;
    let params = json!({ "frameId": frame, "worldName": WORLD_NAME, "grantUniveralAccess": false });
    let world: Value = serde_json::from_str(&host.cdp_send(
        view,
        "Page.createIsolatedWorld",
        &params.to_string(),
    )?)
    .map_err(|_| crate::coded(FailureKind::Other, "errors.frame"))?;
    world["executionContextId"]
        .as_i64()
        .ok_or_else(|| crate::coded(FailureKind::Other, "errors.frame"))
}

/// Runs `job` in `view`: the view's outcome.
fn run_view(host: &mut dyn PluginHost, view: ViewId, job: &Job<'_>) -> Value {
    let mut params = json!({
        "expression": script::wrap(job.code, job.timeout_ms),
        "returnByValue": true,
        "awaitPromise": true,
        "silent": true,
        "userGesture": false,
        "timeout": job.timeout_ms,
    });
    match job.world {
        World::Main => params["includeCommandLineAPI"] = json!(true),
        World::Isolated => match isolated_context(host, view) {
            Ok(context) => params["contextId"] = json!(context),
            Err(failure) => return from_failure(&failure),
        },
    }
    match host.cdp_send(view, "Runtime.evaluate", &params.to_string()) {
        Ok(answer) => outcome_of(&answer),
        Err(failure) => from_failure(&failure),
    }
}

/// Runs `job` in `views` (`known` the grid's views): `{ views, results: [{ ok, type, text,
/// value?, stack?, timedOut?, views }] }`, one result per distinct outcome in the order the
/// views answered.
pub fn run(
    host: &mut dyn PluginHost,
    known: &[ViewInfo],
    views: &[ViewId],
    job: &Job<'_>,
) -> Value {
    let started = host.now_unix_seconds();
    let needs = u64::from(job.timeout_ms).div_ceil(1000) + 1;
    let mut results: Vec<Value> = Vec::new();
    for (done, &view) in views.iter().enumerate() {
        let outcome = match known.iter().find(|info| info.id == view) {
            None => crate::coded_outcome("errors.unknown_view"),
            Some(info) if info.engine != "chromium" => {
                failure_outcome("plugins.error.not_available", false)
            }
            Some(_) if host.now_unix_seconds().saturating_sub(started) + needs > BUDGET_SECONDS => {
                crate::coded_outcome("errors.budget")
            }
            Some(_) => run_view(host, view, job),
        };
        join(&mut results, outcome, view);
        host.report_progress(
            u32::try_from(done + 1).unwrap_or(u32::MAX),
            u32::try_from(views.len()).unwrap_or(u32::MAX),
        );
    }
    json!({ "views": views, "results": results })
}

/// Adds `outcome` of `view` to `results`: to an equal one's views, else as a new result.
fn join(results: &mut Vec<Value>, mut outcome: Value, view: ViewId) {
    let same = |result: &Value| {
        result["ok"] == outcome["ok"]
            && result["text"] == outcome["text"]
            && result["type"] == outcome["type"]
    };
    if let Some(result) = results.iter_mut().find(|result| same(result)) {
        if let Some(views) = result["views"].as_array_mut() {
            views.push(json!(view));
        }
        return;
    }
    outcome["views"] = json!([view]);
    results.push(outcome);
}

#[cfg(test)]
#[path = "run_tests.rs"]
mod tests;
