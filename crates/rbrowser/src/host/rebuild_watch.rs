//! Why the window re-attached its views (RB-099-07). Every grid sent to the window, the
//! DevTools dock laid out anew and the plugin panels' dock laid out anew take every browser view
//! out of its parent and put it back, which Chromium answers with a burst of
//! `StackChildLayerRelativeTo` warnings in `cef.log`. A loop that did so with every frame for
//! two hours left no line in `rbrowser.log`; more than [`LIMIT`] within [`WINDOW`] are told once
//! per [`QUIET`], with the causes of that second counted - the place that rebuilt the grid, the
//! footer that changed its height, the dock - so the next log names the loop.

use std::collections::{BTreeMap, VecDeque};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use rb_diag::BurstWatch;

/// The window the re-attachments are counted in.
const WINDOW: Duration = Duration::from_secs(1);
/// More within [`WINDOW`] are a loop.
const LIMIT: usize = 10;
/// A loop that goes on is told once per this time.
const QUIET: Duration = Duration::from_secs(60);

/// The pace of the re-attachments and their causes within [`WINDOW`].
pub(crate) struct Watch {
    pace: BurstWatch,
    causes: VecDeque<(Instant, String)>,
}

impl Watch {
    const fn new() -> Self {
        Self {
            pace: BurstWatch::new(WINDOW, LIMIT, QUIET),
            causes: VecDeque::new(),
        }
    }

    /// Notes a re-attachment for `cause` at `now`; the causes of the window counted, most
    /// frequent first, when they are a loop to be told.
    pub(crate) fn note(&mut self, now: Instant, cause: String) -> Option<String> {
        while self
            .causes
            .front()
            .is_some_and(|(at, _)| now.saturating_duration_since(*at) >= WINDOW)
        {
            self.causes.pop_front();
        }
        self.causes.push_back((now, cause));
        self.pace.note(now)?;
        let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
        for (_, cause) in &self.causes {
            *counts.entry(cause.as_str()).or_default() += 1;
        }
        let mut counts: Vec<(&str, usize)> = counts.into_iter().collect();
        counts.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
        let told: Vec<String> = counts
            .iter()
            .map(|(cause, count)| format!("{count}x {cause}"))
            .collect();
        Some(told.join("; "))
    }
}

static WATCH: Mutex<Watch> = Mutex::new(Watch::new());

/// The window re-attaches its views for `cause`: a warning when that happens in a loop.
pub(crate) fn note(cause: String) {
    let told = rb_util::lock(&WATCH).note(Instant::now(), cause);
    if let Some(causes) = told {
        tracing::warn!(
            %causes,
            "the window re-attached its views again and again within a second - a loop"
        );
    }
}

#[cfg(test)]
#[path = "rebuild_watch_tests.rs"]
mod tests;
