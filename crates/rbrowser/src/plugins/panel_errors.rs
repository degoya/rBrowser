//! A plugin panel's failed call (RB-080-90): the host texts a panel needs to say it as a
//! sentence, and the line in the app's log.
//!
//! - A panel gets its plugin's catalogue and, with it, [`host_texts`]: the core's text of every
//!   host code ([`codes::ALL`]) and of the reasons a host failure names (`reason=timeout`, ...),
//!   so `texts.js` (`PanelTexts.errorText`) says "rBrowser could not run the plugin call. The view
//!   did not answer in time ..." and never `plugins.error.host_failed`.
//! - Every failure a panel is answered with goes into the log with its code and parameters
//!   ([`log_failure`]): once per plugin, call, code and reason a minute, its text clipped and with
//!   control characters spelled out; the log itself masks secrets (`rb-diag`).

use std::collections::{BTreeMap, HashMap};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use rb_plugin_types::{Failure, codes};

use super::commands::failure_text;

/// The core texts a panel gets besides the host codes': the panels window's own codes, the
/// reasons of a host failure, the waiting line and the sentence of a code without a text.
pub const PANEL_KEYS: &[&str] = &[
    "plugins.error.busy",
    "plugins.error.no_answer",
    "plugins.reason.timeout",
    "plugins.reason.frame",
    "plugins.reason.world",
    "plugins.reason.exception",
    "plugins.reason.no_answer",
    "plugins.panel.waiting",
    "plugins.panel.call_failed",
];

/// How long the same failure stays out of the log after it was written.
pub const LOG_EVERY: Duration = Duration::from_secs(60);

/// The most characters of a failure's text the log keeps.
const LOG_MAX_CHARS: usize = 300;

/// The core's texts of [`codes::ALL`] and [`PANEL_KEYS`], by key; a key without a text is left
/// out (the panel then says [`PANEL_KEYS`]' `plugins.panel.call_failed`, never the key).
#[must_use]
pub fn host_texts() -> BTreeMap<String, String> {
    codes::ALL
        .iter()
        .chain(PANEL_KEYS)
        .filter_map(|key| {
            let text = rb_i18n::t(key);
            (text != *key).then(|| ((*key).to_owned(), text))
        })
        .collect()
}

/// Which failures were written when, so the same one is written once per [`LOG_EVERY`].
#[derive(Debug, Default)]
pub struct Throttle {
    written: Mutex<HashMap<String, Instant>>,
}

impl Throttle {
    /// Whether the failure `key` is written at `now`: the first time, and again after
    /// [`LOG_EVERY`].
    pub fn allows(&self, key: &str, now: Instant) -> bool {
        let mut written = crate::cdp::lock(&self.written);
        // The map holds what was written in the last minute only.
        written.retain(|_, at| now.saturating_duration_since(*at) < LOG_EVERY);
        if written.contains_key(key) {
            return false;
        }
        written.insert(key.to_owned(), now);
        true
    }
}

/// The key a failure of `plugin`'s `call` is throttled by: plugin, call, code and reason.
#[must_use]
pub fn throttle_key(plugin: &str, call: &str, failure: &Failure) -> String {
    let reason = failure.params.get("reason").map_or("", String::as_str);
    let code = failure.code().unwrap_or(codes::HOST_FAILED);
    format!("{plugin}/{call}/{code}/{reason}")
}

/// Writes the failure of `plugin`'s panel call `call` into the log (throttled, clipped).
pub fn log_failure(plugin: &str, call: &str, failure: &Failure) {
    static THROTTLE: OnceLock<Throttle> = OnceLock::new();
    let key = throttle_key(plugin, call, failure);
    if THROTTLE
        .get_or_init(Throttle::default)
        .allows(&key, Instant::now())
    {
        let text = rb_plugin_host::readable(&failure_text(failure), LOG_MAX_CHARS);
        tracing::warn!(plugin, call, "a plugin panel's call failed: {text}");
    }
}

#[cfg(test)]
#[path = "panel_errors_tests.rs"]
mod tests;
