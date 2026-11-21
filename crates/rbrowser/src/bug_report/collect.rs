//! What went wrong in the report's views (RB-093-04), from the window's buffers
//! (`crate::plugins::console::current`, the shapes of `rb_mcp::logs`): the console errors and
//! warnings and the uncaught exceptions, and the failed requests (status 400 and above, or
//! aborted), each deduplicated over the views with a count and the views that saw it. Only the
//! named fields are read - a request's headers or body never, whatever the feed holds - and every
//! text and address passes the [`Masker`] before it is compared, so a token differing per request
//! does not split one problem in two.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use super::mask::Masker;

/// The most distinct console problems and the most distinct failed requests a report lists.
pub const MAX_ITEMS: usize = 200;
/// The longest text kept of one console problem or failure reason, in characters.
pub const MAX_TEXT_CHARS: usize = 2000;
/// The longest method kept (`PROPFIND`).
const MAX_METHOD_CHARS: usize = 16;

/// How bad a console problem is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Level {
    /// `console.error`, a browser error, an uncaught exception.
    Error,
    /// `console.warn`, a browser warning.
    Warning,
}

/// One console problem, as often as it came.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConsoleProblem {
    /// Error or warning.
    pub level: Level,
    /// The message, masked.
    pub text: String,
    /// The script's address and line, masked.
    pub source: Option<String>,
    /// How often it came over all views.
    pub count: u32,
    /// The views it came in.
    pub views: BTreeSet<u32>,
}

/// One failed request, as often as it failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FailedRequest {
    /// `GET`, `POST`, ...; `?` for anything that is no method.
    pub method: String,
    /// The address, masked.
    pub url: String,
    /// The HTTP status, when a response came.
    pub status: Option<u64>,
    /// Why it failed without one (`net::ERR_ABORTED`), masked.
    pub failure: Option<String>,
    /// How often it failed over all views.
    pub count: u32,
    /// The views it failed in.
    pub views: BTreeSet<u32>,
}

/// The problems of a report.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Problems {
    /// The console problems, in the order they first came.
    pub console: Vec<ConsoleProblem>,
    /// The failed requests, in the order they first failed.
    pub requests: Vec<FailedRequest>,
    /// Distinct problems left out beyond [`MAX_ITEMS`].
    pub dropped: usize,
}

/// The problems of the views `views` in `feed` (`{"views": [{"view", "console", "errors",
/// "network"}]}`), masked by `masker`.
pub fn collect(feed: &Value, views: &[u32], masker: &Masker) -> Problems {
    let mut problems = Problems::default();
    let mut console_at: BTreeMap<(Level, String, Option<String>), usize> = BTreeMap::new();
    let mut request_at: BTreeMap<(String, String, Option<u64>, Option<String>), usize> =
        BTreeMap::new();
    let entries = feed["views"].as_array().map_or(&[][..], Vec::as_slice);
    for entry in entries {
        let Some(view) = entry["view"]
            .as_u64()
            .and_then(|id| u32::try_from(id).ok())
            .filter(|id| views.contains(id))
        else {
            continue;
        };
        let console = list(&entry["console"]).iter().filter_map(|line| {
            let level = match line["level"].as_str()? {
                "error" | "assert" => Level::Error,
                "warning" | "warn" => Level::Warning,
                _ => return None,
            };
            Some((level, line))
        });
        let exceptions = list(&entry["errors"])
            .iter()
            .map(|line| (Level::Error, line));
        for (level, line) in console.chain(exceptions) {
            let Some(text) = line["text"].as_str() else {
                continue;
            };
            let text = bounded(&masker.text(text));
            let source = line["url"].as_str().map(|url| {
                let url = masker.text(url);
                match line["line"].as_u64() {
                    Some(number) => format!("{url}:{number}"),
                    None => url,
                }
            });
            let key = (level, text.clone(), source.clone());
            add(
                &mut problems.console,
                &mut console_at,
                &mut problems.dropped,
                key,
                view,
                || ConsoleProblem {
                    level,
                    text,
                    source,
                    count: 0,
                    views: BTreeSet::new(),
                },
            );
        }
        for request in list(&entry["network"]) {
            let Some(url) = request["url"].as_str() else {
                continue;
            };
            let status = request["status"].as_u64();
            let failure = request["failed"]
                .as_str()
                .filter(|reason| !reason.is_empty())
                .map(|reason| bounded(&masker.text(reason)));
            if !(status.is_some_and(|status| status >= 400) || failure.is_some()) {
                continue;
            }
            let method = method(request["method"].as_str().unwrap_or_default());
            let url = masker.text(url);
            let key = (method.clone(), url.clone(), status, failure.clone());
            add(
                &mut problems.requests,
                &mut request_at,
                &mut problems.dropped,
                key,
                view,
                || FailedRequest {
                    method,
                    url,
                    status,
                    failure,
                    count: 0,
                    views: BTreeSet::new(),
                },
            );
        }
    }
    problems
}

/// Counts one more of `key` in `view`, adding it with `make` when new and there is room.
fn add<K: Ord, T: Counted>(
    items: &mut Vec<T>,
    index: &mut BTreeMap<K, usize>,
    dropped: &mut usize,
    key: K,
    view: u32,
    make: impl FnOnce() -> T,
) {
    let at = match index.get(&key) {
        Some(at) => *at,
        None if items.len() >= MAX_ITEMS => {
            *dropped += 1;
            return;
        }
        None => {
            items.push(make());
            index.insert(key, items.len() - 1);
            items.len() - 1
        }
    };
    if let Some(item) = items.get_mut(at) {
        item.count_in(view);
    }
}

trait Counted {
    fn count_in(&mut self, view: u32);
}

impl Counted for ConsoleProblem {
    fn count_in(&mut self, view: u32) {
        self.count = self.count.saturating_add(1);
        self.views.insert(view);
    }
}

impl Counted for FailedRequest {
    fn count_in(&mut self, view: u32) {
        self.count = self.count.saturating_add(1);
        self.views.insert(view);
    }
}

fn list(value: &Value) -> &[Value] {
    value.as_array().map_or(&[][..], Vec::as_slice)
}

/// `text` cut to [`MAX_TEXT_CHARS`] characters, `...` where it was cut.
fn bounded(text: &str) -> String {
    match text.char_indices().nth(MAX_TEXT_CHARS) {
        Some((at, _)) => format!("{}...", &text[..at]),
        None => text.to_owned(),
    }
}

/// `method` when it is a token of ASCII letters, else `?`.
fn method(method: &str) -> String {
    let fine = !method.is_empty()
        && method.len() <= MAX_METHOD_CHARS
        && method.chars().all(|c| c.is_ascii_alphabetic());
    if fine {
        method.to_ascii_uppercase()
    } else {
        "?".to_owned()
    }
}

#[cfg(test)]
#[path = "collect_tests.rs"]
mod tests;
