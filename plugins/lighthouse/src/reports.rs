//! The report history (RB-091-02): the tool `reports` (MCP `lighthouse_reports`: `list`, `open`,
//! `compare`), the commands `open_report` and `delete_run`, and the retention after a run.
//!
//! The plugin cannot open a window or delete a file: an answer asks the host with `$report`
//! (`{"open": "<run>/<file>.html", "window": ...}`, `{"remove": ["<run>"]}`), which the core
//! carries out after its own checks - only below this plugin's out folder, only in a trusted
//! project (`crates/rbrowser/src/report_view/`). Deleting is a person's act: the command (the
//! panel's history, a shortcut), never the MCP tool; the retention
//! (`lighthouse.keepRuns`) removes the oldest runs after each run.

use rb_plugin_api::{Failure, FailureKind, PluginHost};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::history::{self, active_breakpoint, is_run_id};
use crate::run::{LATEST, out_path};
use crate::{PLUGIN_ID, bad_args, error, settings};

/// The answer key of a host request (`crates/rbrowser/src/report_view/ask.rs`).
pub const REPORT_KEY: &str = "$report";

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Args {
    action: Option<String>,
    run: Option<String>,
    breakpoint: Option<String>,
    runs: Option<Vec<String>>,
    target: Option<Value>,
}

fn parse(args: &str) -> Result<Args, Failure> {
    let parsed: Args = if args.trim().is_empty() {
        Args::default()
    } else {
        serde_json::from_str(args).map_err(|error| bad_args(&error.to_string()))?
    };
    if let Some(run) = &parsed.run
        && !is_run_id(run)
    {
        return Err(bad_args("run: a run id, YYYYMMDD-HHMMSS"));
    }
    if let Some(breakpoint) = &parsed.breakpoint
        && (breakpoint.is_empty() || breakpoint.len() > 64)
    {
        return Err(bad_args("breakpoint: 1 to 64 characters"));
    }
    Ok(parsed)
}

/// The breakpoint asked for: named, else the `target` view's.
fn breakpoint_of(host: &mut dyn PluginHost, args: &Args) -> Option<String> {
    if args.breakpoint.is_some() {
        return args.breakpoint.clone();
    }
    let view = args.target.as_ref()?.as_u64()?;
    host.views()
        .ok()?
        .into_iter()
        .find(|found| u64::from(found.id) == view)
        .map(|found| found.breakpoint)
        .filter(|breakpoint| !breakpoint.is_empty())
}

fn no_run(run: &str) -> Failure {
    error(FailureKind::NotFound, "no_run").with_param("run", run)
}

/// `reports`: `list` (the default), `open` or `compare`.
///
/// # Errors
///
/// Arguments outside the schema (`errors.bad_args`), or `open`'s and `compare`'s.
pub fn tool(host: &mut dyn PluginHost, args: &str) -> Result<String, Failure> {
    let args = parse(args)?;
    let breakpoint = breakpoint_of(host, &args);
    let answer = match args.action.as_deref() {
        None | Some("list") => history::list_answer(host),
        Some("open") => open(host, args.run.as_deref(), breakpoint.as_deref())?,
        Some("compare") => history::compare(host, breakpoint.as_deref(), args.runs.as_deref())?,
        Some(_) => return Err(bad_args("action: list, open or compare")),
    };
    Ok(answer.to_string())
}

/// The command `open_report`: as the tool's `open`.
///
/// # Errors
///
/// As [`tool`].
pub fn open_command(host: &mut dyn PluginHost, args: &str) -> Result<String, Failure> {
    let args = parse(args)?;
    let breakpoint = breakpoint_of(host, &args);
    Ok(open(host, args.run.as_deref(), breakpoint.as_deref())?.to_string())
}

/// The report of `breakpoint` (else the active view's, else the first) in run `run` (else the
/// newest), as a host request to open it.
fn open(
    host: &mut dyn PluginHost,
    run: Option<&str>,
    breakpoint: Option<&str>,
) -> Result<Value, Failure> {
    let runs = history::list(host);
    let chosen = match run {
        Some(id) => runs
            .iter()
            .find(|found| found.id == id)
            .ok_or_else(|| no_run(id))?,
        None => runs
            .first()
            .ok_or_else(|| error(FailureKind::NotFound, "no_reports"))?,
    };
    let entry = match breakpoint {
        Some(name) => chosen.breakpoints.iter().find(|entry| entry.name == name),
        None => {
            let active = active_breakpoint(host);
            active
                .and_then(|name| chosen.breakpoints.iter().find(|entry| entry.name == name))
                .or_else(|| chosen.breakpoints.first())
        }
    }
    .ok_or_else(|| {
        error(FailureKind::NotFound, "no_report")
            .with_param("run", &chosen.id)
            .with_param("breakpoint", breakpoint.unwrap_or_default())
    })?;
    Ok(json!({
        "run": chosen.id, "breakpoint": entry.name, "report": entry.report,
        REPORT_KEY: { "open": entry.report, "window": settings::report_window(host) },
    }))
}

/// The command `delete_run`: the run `run` goes (the host removes its folder); without one the
/// panel opens, where the history asks which. A deleted latest run makes the newest one left
/// the latest (`latest.json`, `null` when none is).
///
/// # Errors
///
/// A run id that is none (`errors.bad_args`) or not on disk (`errors.no_run`), or
/// `latest.json` cannot be written.
pub fn delete_command(host: &mut dyn PluginHost, args: &str) -> Result<String, Failure> {
    let args = parse(args)?;
    let Some(run) = args.run else {
        return Ok(json!({ "$open": { "panel": PLUGIN_ID } }).to_string());
    };
    let runs = history::list(host);
    if !runs.iter().any(|found| found.id == run) {
        return Err(no_run(&run));
    }
    let latest = host
        .project_read(&out_path(LATEST))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok());
    if latest.as_ref().and_then(|latest| latest["runId"].as_str()) == Some(run.as_str()) {
        let next = runs.iter().find(|found| found.id != run).and_then(|found| {
            host.project_read(&out_path(&format!("{}/run.json", found.id)))
                .ok()
        });
        host.out_write(LATEST, &next.unwrap_or_else(|| b"null".to_vec()))?;
    }
    Ok(json!({ "deleted": run, REPORT_KEY: { "remove": [run] } }).to_string())
}

/// Adds to a run's `summary` the runs past `lighthouse.keepRuns`, for the host to remove.
pub fn retain(host: &mut dyn PluginHost, summary: &mut Value) {
    let keep = settings::keep_runs(host);
    let old = history::beyond(host, keep);
    if !old.is_empty()
        && let Some(fields) = summary.as_object_mut()
    {
        fields.insert(REPORT_KEY.to_owned(), json!({ "remove": old }));
    }
}

#[cfg(test)]
#[path = "reports_tests.rs"]
mod tests;
