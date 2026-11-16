//! "Report now" with a fresh load: the views reloaded and waited for until their network is idle.
//!
//! Through `evaluate` in the plugin's own world (Chromium views only): a script notes the page's
//! `performance.timeOrigin` and reloads it, then a second script, run once the new document is
//! there (another `timeOrigin`), waits until the document is complete and no resource has started
//! for [`QUIET_MS`], at most [`SETTLE_MS`]. While the old document is still in place, or the
//! context goes away under the script, it is asked again, within [`MAX_ATTEMPTS`] and
//! [`MAX_SECONDS`]. The capture of the call that reloads predates the reload, so the report of the
//! fresh load is the next call's.

use rb_plugin_api::{PluginHost, ViewId};
use serde_json::Value;

/// No new resource for this long counts as network idle.
pub const QUIET_MS: u32 = 500;
/// The longest wait for one view once its new document is there.
pub const SETTLE_MS: u32 = 10_000;
/// The most times the wait script is asked per view.
pub const MAX_ATTEMPTS: u32 = 100;
/// The longest the whole wait takes, over every view.
pub const MAX_SECONDS: u64 = 25;

/// Notes the time origin and reloads once the answer is on its way.
const RELOAD_SCRIPT: &str = "(() => { const origin = performance.timeOrigin; setTimeout(() => location.reload(), 0); return origin; })()";

/// What the reload of one view came to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Outcome {
    pub view: ViewId,
    /// The new document finished and its network went idle in time.
    pub settled: bool,
    /// The failure code when the view could not be reloaded or waited for.
    pub error: Option<String>,
}

/// The wait script for a view whose old document had `before` as its time origin.
#[must_use]
pub fn wait_script(before: Option<f64>) -> String {
    let before = before.map_or_else(|| "null".to_owned(), |origin| origin.to_string());
    format!(
        "(async (before) => {{ const sleep = (ms) => new Promise((done) => setTimeout(done, ms)); \
if (before !== null && performance.timeOrigin === before) {{ await sleep(150); return {{ stale: true }}; }} \
const deadline = Date.now() + {SETTLE_MS}; let count = -1; let quiet = Date.now(); \
while (Date.now() < deadline) {{ const n = performance.getEntriesByType('resource').length; \
if (n !== count) {{ count = n; quiet = Date.now(); }} \
if (document.readyState === 'complete' && Date.now() - quiet >= {QUIET_MS}) return {{ settled: true }}; \
await sleep(100); }} return {{ settled: false }}; }})({before})"
    )
}

fn code(failure: &rb_plugin_api::Failure) -> String {
    failure
        .code
        .clone()
        .unwrap_or_else(|| "plugins.error.host_failed".to_owned())
}

/// Reloads `views` and waits for each to settle.
pub fn reload(host: &mut dyn PluginHost, views: &[ViewId]) -> Vec<Outcome> {
    let mut started: Vec<(ViewId, Result<Option<f64>, String>)> = Vec::new();
    for &view in views {
        let origin = host
            .evaluate(view, RELOAD_SCRIPT)
            .map(|answer| {
                serde_json::from_str::<Value>(&answer)
                    .ok()
                    .and_then(|v| v.as_f64())
            })
            .map_err(|failure| code(&failure));
        started.push((view, origin));
    }
    let deadline = host.now_unix_seconds().saturating_add(MAX_SECONDS);
    started
        .into_iter()
        .map(|(view, origin)| match origin {
            Err(error) => Outcome {
                view,
                settled: false,
                error: Some(error),
            },
            Ok(before) => wait(host, view, before, deadline),
        })
        .collect()
}

fn wait(host: &mut dyn PluginHost, view: ViewId, before: Option<f64>, deadline: u64) -> Outcome {
    let script = wait_script(before);
    let mut error = None;
    for _ in 0..MAX_ATTEMPTS {
        if host.now_unix_seconds() > deadline {
            break;
        }
        match host.evaluate(view, &script) {
            Ok(answer) => {
                let answer: Value = serde_json::from_str(&answer).unwrap_or(Value::Null);
                if answer["stale"] == Value::Bool(true) {
                    continue;
                }
                return Outcome {
                    view,
                    settled: answer["settled"] == Value::Bool(true),
                    error: None,
                };
            }
            // The context went away with the old document, or the new one is not ready yet.
            Err(failure) => error = Some(code(&failure)),
        }
    }
    Outcome {
        view,
        settled: false,
        error,
    }
}

#[cfg(test)]
#[path = "reload_tests.rs"]
mod tests;
