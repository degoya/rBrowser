//! The page side, through `evaluate` in the plugin's isolated world: the style sheets the
//! document lists, a reload, and the wait until the reloaded page is loaded and its network idle.
//! No page script is injected; each script is one expression and leaves nothing behind.

use rb_plugin_api::{Failure, FailureKind, PluginHost, ViewId, codes};
use serde::Deserialize;
use serde_json::Value;

/// The most sheets taken from one page.
pub const MAX_SHEETS: usize = 500;
/// How long the wait script waits per call, in milliseconds, before it answers `pending`.
pub const WAIT_SLICE_MS: u64 = 5_000;

/// One style sheet of the document (`document.styleSheets`, `@import`ed ones where readable).
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
pub struct PageSheet {
    /// Its URL; none for a `<style>` element.
    #[serde(default)]
    pub href: Option<String>,
    /// A `<style>` element's text.
    #[serde(default)]
    pub text: Option<String>,
}

/// The document's URL and sheets.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
pub struct Sheets {
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub sheets: Vec<PageSheet>,
}

/// Lists the sheets: linked ones by URL, `<style>` elements with their text, `@import`s of
/// readable sheets; at most `MAX_SHEETS`.
pub const SHEETS: &str = r"(() => {
  const out = [];
  const seen = new Set();
  const walk = (sheet, depth) => {
    if (!sheet || seen.has(sheet) || depth > 8 || out.length >= 500) return;
    seen.add(sheet);
    const node = sheet.ownerNode;
    if (sheet.href) out.push({ href: sheet.href });
    else out.push({ text: node && typeof node.textContent === 'string' ? node.textContent : '' });
    let rules = null;
    try { rules = sheet.cssRules; } catch (_) { rules = null; }
    if (!rules) return;
    for (const rule of rules) if (rule.type === 3 && rule.styleSheet) walk(rule.styleSheet, depth + 1);
  };
  for (const sheet of document.styleSheets) walk(sheet, 0);
  return { url: location.href, sheets: out };
})()";

/// Reloads the page on the next task and answers the current document's time origin, which
/// tells the reloaded document from this one.
pub const RELOAD: &str = r"(() => {
  const origin = performance.timeOrigin;
  setTimeout(() => location.reload(), 0);
  return origin;
})()";

/// The wait script: `old` while the document of `origin` is still there, `ready` once the
/// reloaded one has loaded and seen no new request for `idle` ms, `pending` after a slice.
#[must_use]
pub fn wait_script(origin: f64, idle_ms: u64) -> String {
    format!(
        r"(async () => {{
  const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
  if (performance.timeOrigin === {origin}) {{ await sleep(200); return {{ state: 'old' }}; }}
  const until = performance.now() + {WAIT_SLICE_MS};
  if (document.readyState !== 'complete') {{
    await Promise.race([new Promise((resolve) => addEventListener('load', resolve, {{ once: true }})), sleep({WAIT_SLICE_MS})]);
    if (document.readyState !== 'complete') return {{ state: 'pending' }};
  }}
  let count = performance.getEntriesByType('resource').length;
  let quiet = performance.now();
  while (performance.now() - quiet < {idle_ms}) {{
    if (performance.now() > until) return {{ state: 'pending' }};
    await sleep(50);
    const now = performance.getEntriesByType('resource').length;
    if (now !== count) {{ count = now; quiet = performance.now(); }}
  }}
  return {{ state: 'ready', url: location.href }};
}})()"
    )
}

/// What the wait script answered.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Wait {
    /// The old document is still shown.
    Old,
    /// Loading, or requests still coming.
    Pending,
    /// Loaded and idle, at this URL.
    Ready(String),
}

fn bad(reason: &str) -> Failure {
    Failure::coded(FailureKind::BadData, codes::BAD_OUTPUT).with_param("reason", reason)
}

/// The sheets of `view`'s document.
///
/// # Errors
///
/// The host's failure, or an answer of another shape.
pub fn sheets(host: &mut dyn PluginHost, view: ViewId) -> Result<Sheets, Failure> {
    let answer = host.evaluate(view, SHEETS)?;
    let mut sheets: Sheets = serde_json::from_str(&answer).map_err(|_| bad("sheets"))?;
    sheets.sheets.truncate(MAX_SHEETS);
    Ok(sheets)
}

/// Reloads `view` and answers the time origin of the document it leaves.
///
/// # Errors
///
/// The host's failure, or no number for an answer.
pub fn reload(host: &mut dyn PluginHost, view: ViewId) -> Result<f64, Failure> {
    let answer = host.evaluate(view, RELOAD)?;
    serde_json::from_str::<Value>(&answer)
        .ok()
        .and_then(|value| value.as_f64())
        .ok_or_else(|| bad("reload"))
}

/// One slice of the wait for `view`; a failing call (the old document went away while it ran)
/// is `Pending`.
pub fn wait(host: &mut dyn PluginHost, view: ViewId, origin: f64, idle_ms: u64) -> Wait {
    let Ok(answer) = host.evaluate(view, &wait_script(origin, idle_ms)) else {
        return Wait::Pending;
    };
    let value: Value = serde_json::from_str(&answer).unwrap_or(Value::Null);
    match value["state"].as_str() {
        Some("old") => Wait::Old,
        Some("ready") => Wait::Ready(value["url"].as_str().unwrap_or_default().to_owned()),
        _ => Wait::Pending,
    }
}

#[cfg(test)]
#[path = "page_tests.rs"]
mod tests;
