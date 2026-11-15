//! The watch's errors, once each. Polling walks every watched folder on each pass and reports
//! an entry it cannot read every time (a WSL project's symlink to a container path, a folder
//! without access): such an error is logged and rescanned once, again only when it changed,
//! recovered (an event about its path) or stopped for [`FORGET_AFTER`] and came back.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// How long an error that stopped repeating is remembered.
pub const FORGET_AFTER: Duration = Duration::from_secs(60);

/// The errors seen and still repeating, by their text.
#[derive(Debug, Default)]
pub struct WatchErrors {
    failing: HashMap<String, Failing>,
}

#[derive(Debug)]
struct Failing {
    paths: Vec<PathBuf>,
    last: Instant,
}

impl WatchErrors {
    /// The error `key` (its text) about `paths` at `now`; `true` when it is news (log it,
    /// rescan), `false` for a repeat.
    pub fn on_error(&mut self, key: &str, paths: &[PathBuf], now: Instant) -> bool {
        self.failing
            .retain(|_, failing| now.saturating_duration_since(failing.last) <= FORGET_AFTER);
        if let Some(failing) = self.failing.get_mut(key) {
            failing.last = now;
            return false;
        }
        self.failing.insert(
            key.to_owned(),
            Failing {
                paths: paths.to_vec(),
                last: now,
            },
        );
        true
    }

    /// An event about `paths`: the errors about one of them are over. Their keys.
    pub fn on_event(&mut self, paths: &[PathBuf]) -> Vec<String> {
        let over: Vec<String> = self
            .failing
            .iter()
            .filter(|(_, failing)| failing.paths.iter().any(|path| paths.contains(path)))
            .map(|(key, _)| key.clone())
            .collect();
        for key in &over {
            self.failing.remove(key);
        }
        over
    }
}

#[cfg(test)]
#[path = "watch_errors_tests.rs"]
mod tests;
