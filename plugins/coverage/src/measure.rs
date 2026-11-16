//! The measurement over CDP, per Chromium view. Nothing is left tracking after a run: every path
//! that started CSS rule usage tracking or V8's precise coverage stops it again, a failing one
//! included.
//!
//! - **Load**: precise coverage starts (`Profiler.enable`, `startPreciseCoverage` with call
//!   counts, per function as DevTools counts by default) and is drained once, so the scripts of
//!   the document about to go are known and left out; every view reloads (`location.reload()`
//!   from the plugin's world) and is waited for until loaded and its network idle; then CSS rule
//!   usage is tracked
//!   (`CSS.startRuleUsageTracking` recalculates every element's style, so the rules the loaded
//!   page matches are marked) and stopped at once, and the scripts' coverage is taken and
//!   stopped.
//! - **Interaction**: `start` starts both kinds of tracking, `take` reads what was used since the
//!   last take (`CSS.takeCoverageDelta`, `Profiler.takePreciseCoverage`), `finish` stops both.
//!
//! `Profiler.enable` stays on the shared session (a plugin may send no `*.disable`): with precise
//! coverage stopped, V8 is back to its best-effort coverage and the agent costs nothing.

use std::collections::BTreeSet;

use rb_plugin_api::{Failure, FailureKind, PluginHost, ViewId, ViewInfo, codes};
use serde_json::{Value, json};

use crate::model::{FileRun, Status, ViewRun};
use crate::page::{self, Wait};
use crate::resolve::{self, Cache, RuleUsage, ScriptCoverage};
use crate::settings::Settings;

/// The most wait slices per view (each answers within a few seconds), a stop for a fake clock.
const MAX_WAITS: u32 = 400;

fn send(
    host: &mut dyn PluginHost,
    view: ViewId,
    method: &str,
    params: Value,
) -> Result<Value, Failure> {
    let answer = host.cdp_send(view, method, &params.to_string())?;
    serde_json::from_str(&answer).map_err(|_| {
        Failure::coded(FailureKind::BadData, codes::BAD_OUTPUT).with_param("method", method)
    })
}

/// A view's line of the result before (or without) a measurement.
#[must_use]
pub fn view_run(view: &ViewInfo) -> ViewRun {
    let chromium = view.engine == "chromium";
    ViewRun {
        view: view.id,
        label: view.label.clone(),
        breakpoint: view.breakpoint.clone(),
        width: view.width,
        engine: view.engine.clone(),
        url: view.url.clone(),
        status: if chromium {
            Status::Measured
        } else {
            Status::Unavailable
        },
        error: (!chromium).then(|| codes::NOT_AVAILABLE.to_owned()),
        files: Vec::new(),
    }
}

/// Marks `run` failed with `failure`'s code.
pub fn failed(run: &mut ViewRun, failure: &Failure) {
    run.status = Status::Failed;
    run.error = Some(failure.code().unwrap_or(codes::HOST_FAILED).to_owned());
}

fn start_precise(host: &mut dyn PluginHost, view: ViewId) -> Result<(), Failure> {
    send(host, view, "Profiler.enable", json!({}))?;
    send(
        host,
        view,
        "Profiler.startPreciseCoverage",
        // Per function, as DevTools' coverage counts by default; block detail would multiply
        // the answer of a large bundle past the host's 4 MiB.
        json!({ "callCount": true, "detailed": false }),
    )?;
    Ok(())
}

fn start_css(host: &mut dyn PluginHost, view: ViewId) -> Result<(), Failure> {
    send(host, view, "DOM.enable", json!({}))?;
    send(host, view, "CSS.enable", json!({}))?;
    send(host, view, "CSS.startRuleUsageTracking", json!({}))?;
    Ok(())
}

/// Stops both kinds of tracking in `view`, whatever runs; failures only logged.
pub fn stop(host: &mut dyn PluginHost, view: ViewId) {
    for method in ["CSS.stopRuleUsageTracking", "Profiler.stopPreciseCoverage"] {
        if host.cdp_send(view, method, "{}").is_err() {
            host.log(
                "debug",
                &format!("coverage: {method} in view {view} failed"),
            );
        }
    }
}

fn scripts(value: &Value) -> Vec<ScriptCoverage> {
    serde_json::from_value(value["result"].clone()).unwrap_or_default()
}

fn usage(value: &Value, key: &str) -> Vec<RuleUsage> {
    serde_json::from_value(value[key].clone()).unwrap_or_default()
}

fn take_scripts(host: &mut dyn PluginHost, view: ViewId) -> Result<Vec<ScriptCoverage>, Failure> {
    send(host, view, "Profiler.takePreciseCoverage", json!({})).map(|value| scripts(&value))
}

/// Files of one view from what was taken.
fn files(
    host: &mut dyn PluginHost,
    view: ViewId,
    rules: &[RuleUsage],
    scripts: &[ScriptCoverage],
    skip: &BTreeSet<String>,
    settings: &Settings,
    cache: &mut Cache,
) -> Result<(String, Vec<FileRun>), Failure> {
    let (url, mut files) = resolve::css(host, view, rules, settings, cache)?;
    files.extend(resolve::js(scripts, &url, skip, settings));
    Ok((url, files))
}

/// One view of a load run.
struct Loading {
    run: ViewRun,
    /// Precise coverage is on and not stopped yet.
    precise: bool,
    /// The script ids of the document that was reloaded.
    old: BTreeSet<String>,
    origin: f64,
    url: Option<String>,
}

fn wait_all(host: &mut dyn PluginHost, loading: &mut [Loading], settings: &Settings) {
    let deadline = host
        .now_unix_seconds()
        .saturating_add(settings.load_timeout_seconds);
    for _ in 0..MAX_WAITS {
        let mut waiting = false;
        for view in loading.iter_mut() {
            if view.run.status != Status::Measured || view.url.is_some() {
                continue;
            }
            match page::wait(host, view.run.view, view.origin, settings.idle_ms) {
                Wait::Ready(url) => view.url = Some(url),
                Wait::Old | Wait::Pending => waiting = true,
            }
        }
        if !waiting || host.now_unix_seconds() > deadline {
            break;
        }
    }
    for view in loading.iter_mut() {
        if view.run.status == Status::Measured && view.url.is_none() {
            view.run.status = Status::Failed;
            view.run.error = Some(codes::TIMED_OUT.to_owned());
        }
    }
}

/// The load run over `views`: one line per view, Firefox and WebKit views `unavailable`.
pub fn load(host: &mut dyn PluginHost, views: &[ViewInfo], settings: &Settings) -> Vec<ViewRun> {
    let mut loading: Vec<Loading> = views
        .iter()
        .map(|view| Loading {
            run: view_run(view),
            precise: false,
            old: BTreeSet::new(),
            origin: 0.0,
            url: None,
        })
        .collect();
    let total = u32::try_from(loading.len() * 3).unwrap_or(u32::MAX);
    let mut done = 0;
    for view in loading
        .iter_mut()
        .filter(|view| view.run.status == Status::Measured)
    {
        let id = view.run.view;
        view.precise = true;
        let started = start_precise(host, id)
            .and_then(|()| take_scripts(host, id))
            .and_then(|old| {
                view.old = old.into_iter().map(|script| script.script_id).collect();
                page::reload(host, id)
            });
        match started {
            Ok(origin) => view.origin = origin,
            Err(failure) => failed(&mut view.run, &failure),
        }
        done += 1;
        host.report_progress(done, total);
    }
    wait_all(host, &mut loading, settings);
    let mut cache = Cache::default();
    for view in &mut loading {
        done += 2;
        host.report_progress(done.min(total), total);
        if view.run.status != Status::Measured {
            continue;
        }
        let id = view.run.view;
        let rules = start_css(host, id)
            .and_then(|()| send(host, id, "CSS.stopRuleUsageTracking", json!({})))
            .map(|value| usage(&value, "ruleUsage"));
        let taken = take_scripts(host, id);
        view.precise = send(host, id, "Profiler.stopPreciseCoverage", json!({})).is_err();
        if rules.is_err() {
            // CSS tracking may have started: stop both again.
            stop(host, id);
            view.precise = false;
        }
        let measured = rules.and_then(|rules| {
            let scripts = taken?;
            files(host, id, &rules, &scripts, &view.old, settings, &mut cache)
        });
        match measured {
            Ok((url, files)) => {
                view.run.url = url;
                view.run.files = files;
            }
            Err(failure) => failed(&mut view.run, &failure),
        }
    }
    // Every path ends here: whatever still tracks is stopped.
    for view in &loading {
        if view.precise {
            stop(host, view.run.view);
        }
    }
    loading.into_iter().map(|view| view.run).collect()
}

/// Starts both kinds of tracking in every Chromium view of `views`; a view that fails is stopped
/// again and marked failed.
pub fn start(host: &mut dyn PluginHost, views: &[ViewInfo]) -> Vec<ViewRun> {
    let mut runs = Vec::new();
    for view in views {
        let mut run = view_run(view);
        if run.status == Status::Measured
            && let Err(failure) =
                start_css(host, view.id).and_then(|()| start_precise(host, view.id))
        {
            stop(host, view.id);
            failed(&mut run, &failure);
        }
        runs.push(run);
    }
    runs
}

/// What was used in `view` since the last take; with `finish`, both kinds of tracking end
/// (stopped also when reading fails).
pub fn take(
    host: &mut dyn PluginHost,
    view: &ViewRun,
    finish: bool,
    settings: &Settings,
    cache: &mut Cache,
) -> ViewRun {
    let mut run = ViewRun {
        files: Vec::new(),
        ..view.clone()
    };
    if view.status != Status::Measured {
        return run;
    }
    let id = view.view;
    let (method, key) = if finish {
        ("CSS.stopRuleUsageTracking", "ruleUsage")
    } else {
        ("CSS.takeCoverageDelta", "coverage")
    };
    let rules = send(host, id, method, json!({})).map(|value| usage(&value, key));
    let scripts = take_scripts(host, id);
    if finish {
        let stopped = send(host, id, "Profiler.stopPreciseCoverage", json!({})).is_ok();
        if rules.is_err() || !stopped {
            stop(host, id);
        }
    }
    let measured = rules.and_then(|rules| {
        let scripts = scripts?;
        files(
            host,
            id,
            &rules,
            &scripts,
            &BTreeSet::new(),
            settings,
            cache,
        )
    });
    match measured {
        Ok((url, files)) => {
            run.url = url;
            run.files = files;
        }
        Err(failure) => failed(&mut run, &failure),
    }
    run
}

#[cfg(test)]
#[path = "measure_tests.rs"]
mod tests;
