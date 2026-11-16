//! The plugin's side of the sidecar's `lighthouse.*` methods
//! (`sidecar/src/modules/lighthouse/index.ts`): start a job, poll it until it ends, fetch its
//! reports slice by slice, release it. A poll waits at most [`POLL_WAIT_MS`] inside the sidecar,
//! well below the sidecar's request timeout (`engines.nodeRpcTimeoutMs`), so a Lighthouse run of
//! a minute is many short requests; a job that runs longer than [`MAX_JOB_SECONDS`] is
//! cancelled.
//!
//! The host's failures become the plugin's own: no sidecar (no Node runtime, it does not start)
//! is `errors.no_sidecar` naming Settings > Engines > Node runtime; no browser to run Lighthouse
//! in is `errors.no_browser`, the browser set in `engines.lighthouseBrowserPath` missing is
//! `errors.browser_missing` (RB-080-58); anything else the sidecar refused is
//! `errors.sidecar_failed`.

use rb_plugin_api::{Failure, FailureKind, PluginHost, codes};
use serde_json::{Value, json};

use crate::error;

/// How long one poll waits in the sidecar for the job to end.
pub const POLL_WAIT_MS: u64 = 4_000;
/// The longest one Lighthouse run may take.
pub const MAX_JOB_SECONDS: u64 = 240;
/// The largest report the plugin saves: `out.write`'s limit.
pub const MAX_REPORT_BYTES: u64 = rb_plugin_api::MAX_IO_BYTES as u64;
/// The sidecar's code for "no browser" (`LighthouseCode.NoBrowser`).
const NO_BROWSER: &str = "(-32010)";
/// The sidecar's code for "the set browser does not exist" (`LighthouseCode.BrowserMissing`).
const BROWSER_MISSING: i64 = -32015;

/// `errors.browser_missing` with the sidecar's reason (the path, profile folders masked).
fn browser_missing(reason: &str) -> Failure {
    error(FailureKind::NotFound, "browser_missing").with_param("reason", reason)
}

/// How a job ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Ended {
    Done {
        final_url: String,
        json_bytes: u64,
        html_bytes: u64,
    },
    Failed {
        message: String,
    },
    Cancelled,
}

/// The plugin's failure for a failed sidecar call.
#[must_use]
pub fn map_failure(failure: Failure) -> Failure {
    let reason = failure.param("reason").unwrap_or_default().to_owned();
    match failure.code() {
        Some(codes::NOT_AVAILABLE) => {
            error(FailureKind::NotSupported, "no_sidecar").with_param("reason", "not started")
        }
        Some(codes::HOST_FAILED) if reason.contains(NO_BROWSER) => {
            error(FailureKind::NotSupported, "no_browser")
        }
        Some(codes::HOST_FAILED) if reason.contains(&format!("({BROWSER_MISSING}): ")) => {
            let marker = format!("({BROWSER_MISSING}): ");
            let detail = reason.split_once(&marker).map_or("", |(_, rest)| rest);
            browser_missing(detail)
        }
        // An answer of the sidecar reads "`method` failed (code): message"; anything else is
        // the sidecar itself failing: no Node, not starting, exited.
        Some(codes::HOST_FAILED) if !reason.contains("` failed (") && reason != "timeout" => {
            error(FailureKind::NotSupported, "no_sidecar").with_param("reason", reason)
        }
        Some(codes::HOST_FAILED) => {
            error(FailureKind::Other, "sidecar_failed").with_param("reason", reason)
        }
        _ => failure,
    }
}

fn call(host: &mut dyn PluginHost, method: &str, params: &Value) -> Result<Value, Failure> {
    let answer = host
        .sidecar_call(method, &params.to_string())
        .map_err(map_failure)?;
    serde_json::from_str(&answer).map_err(|e| {
        error(FailureKind::BadData, "sidecar_failed").with_param("reason", e.to_string())
    })
}

fn bad_answer(method: &str) -> Failure {
    error(FailureKind::BadData, "sidecar_failed")
        .with_param("reason", format!("{method} answered something unexpected"))
}

/// Starts a Lighthouse job; answers its id.
///
/// # Errors
///
/// The sidecar is not there or refused the job.
pub fn start(
    host: &mut dyn PluginHost,
    url: &str,
    label: &str,
    settings: &Value,
) -> Result<String, Failure> {
    let answer = call(
        host,
        "lighthouse.start",
        &json!({ "url": url, "label": label, "settings": settings }),
    )?;
    answer["job"]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| bad_answer("lighthouse.start"))
}

/// Polls `job` until it ends; reports `progress` while it waits.
///
/// # Errors
///
/// A poll failed, or the job took longer than [`MAX_JOB_SECONDS`] (it is cancelled then).
pub fn wait(host: &mut dyn PluginHost, job: &str, breakpoint: &str) -> Result<Ended, Failure> {
    let started = host.now_unix_seconds();
    loop {
        let answer = call(
            host,
            "lighthouse.poll",
            &json!({ "job": job, "waitMs": POLL_WAIT_MS }),
        )?;
        match answer["state"].as_str() {
            Some("done") => {
                let result = &answer["result"];
                return Ok(Ended::Done {
                    final_url: result["finalUrl"].as_str().unwrap_or_default().to_owned(),
                    json_bytes: result["jsonBytes"].as_u64().unwrap_or(0),
                    html_bytes: result["htmlBytes"].as_u64().unwrap_or(0),
                });
            }
            Some("failed") => {
                let error = &answer["error"];
                if error["code"] == -32010 {
                    return Err(crate::error(FailureKind::NotSupported, "no_browser"));
                }
                if error["code"] == BROWSER_MISSING {
                    return Err(browser_missing(
                        error["message"].as_str().unwrap_or_default(),
                    ));
                }
                let message = error["message"].as_str().unwrap_or_default().to_owned();
                return Ok(Ended::Failed { message });
            }
            Some("cancelled") => return Ok(Ended::Cancelled),
            Some("queued" | "running") => {}
            _ => return Err(bad_answer("lighthouse.poll")),
        }
        if host.now_unix_seconds().saturating_sub(started) > MAX_JOB_SECONDS {
            let _ = call(host, "lighthouse.release", &json!({ "job": job }));
            return Err(error(FailureKind::Other, "timed_out")
                .with_param("seconds", MAX_JOB_SECONDS)
                .with_param("breakpoint", breakpoint));
        }
    }
}

/// The whole report `format` (`json` or `html`) of a finished job.
///
/// # Errors
///
/// A slice failed or the answers do not add up.
pub fn report(host: &mut dyn PluginHost, job: &str, format: &str) -> Result<String, Failure> {
    let mut text = String::new();
    let mut offset = 0_u64;
    loop {
        let answer = call(
            host,
            "lighthouse.report",
            &json!({ "job": job, "format": format, "offset": offset }),
        )?;
        let slice = answer["text"]
            .as_str()
            .ok_or_else(|| bad_answer("lighthouse.report"))?;
        text.push_str(slice);
        if text.len() as u64 > MAX_REPORT_BYTES {
            return Err(bad_answer("lighthouse.report"));
        }
        match answer["next"].as_u64() {
            Some(next) if next > offset => offset = next,
            Some(_) => return Err(bad_answer("lighthouse.report")),
            None => return Ok(text),
        }
    }
}

/// Forgets `job` in the sidecar; a failure only logs.
pub fn release(host: &mut dyn PluginHost, job: &str) {
    if let Err(failure) = call(host, "lighthouse.release", &json!({ "job": job })) {
        host.log("warn", &format!("job {job} not released: {failure}"));
    }
}

/// Cancels every run of the sidecar; answers how many.
///
/// # Errors
///
/// The sidecar is not there.
pub fn cancel(host: &mut dyn PluginHost) -> Result<u64, Failure> {
    let answer = call(host, "lighthouse.cancel", &json!({}))?;
    Ok(answer["cancelled"].as_u64().unwrap_or(0))
}

/// The sidecar's state of the runs: `{running, queued}`.
///
/// # Errors
///
/// The sidecar is not there.
pub fn status(host: &mut dyn PluginHost) -> Result<Value, Failure> {
    call(host, "lighthouse.status", &json!({}))
}

#[cfg(test)]
#[path = "sidecar_tests.rs"]
mod tests;
