//! What the state forwarder pushes to the shell's pages (RB-090-36): every page parses and
//! renders each push, so a state like the one sent last goes out no more, and a change of the
//! cell labels' metrics alone - the resources every 2 s, the Web Vitals while a page loads -
//! goes out as `metrics`, a change of the plugins' progress alone (up to 4 Hz) as
//! `pluginProgress` (RB-094-10), not as the whole state.

use std::sync::{Arc, Mutex, Weak};

use rb_ipc::{CoreToShell, StateSnapshot};

use crate::cdp::lock;
use crate::host::Host;

/// The state sent last, and what to send after it.
#[derive(Default)]
pub struct StatePush {
    last: Option<StateSnapshot>,
}

impl StatePush {
    /// The message for `now`: `None` when it is the state sent last, `metrics` when only they
    /// changed, `pluginProgress` when only it changed, else the whole state.
    pub fn next(&mut self, mut now: StateSnapshot) -> Option<CoreToShell> {
        let Some(last) = self.last.as_mut() else {
            return Some(self.whole(now));
        };
        let metrics = std::mem::take(&mut now.metrics);
        let progress = std::mem::take(&mut now.plugin_progress);
        let last_metrics = std::mem::take(&mut last.metrics);
        let last_progress = std::mem::take(&mut last.plugin_progress);
        let rest_same = *last == now;
        now.metrics = metrics;
        now.plugin_progress = progress;
        last.metrics = last_metrics;
        last.plugin_progress = last_progress;
        let metrics_same = last.metrics == now.metrics;
        let progress_same = last.plugin_progress == now.plugin_progress;
        match (rest_same, metrics_same, progress_same) {
            (true, true, true) => None,
            (true, false, true) => {
                last.metrics.clone_from(&now.metrics);
                Some(CoreToShell::Metrics {
                    metrics: now.metrics,
                })
            }
            (true, true, false) => {
                last.plugin_progress.clone_from(&now.plugin_progress);
                Some(CoreToShell::PluginProgress {
                    progress: now.plugin_progress,
                })
            }
            _ => Some(self.whole(now)),
        }
    }

    /// The next state goes out whole, the one sent last or not.
    pub fn forget(&mut self) {
        self.last = None;
    }

    fn whole(&mut self, now: StateSnapshot) -> CoreToShell {
        self.last = Some(now.clone());
        CoreToShell::State(Box::new(now))
    }
}

/// The windows whose next state goes out whole ([`force_next`]).
fn forced() -> &'static Mutex<Vec<Weak<Host>>> {
    static FORCED: Mutex<Vec<Weak<Host>>> = Mutex::new(Vec::new());
    &FORCED
}

/// The next state of `host`'s window goes out whole even when it is the one sent last: the
/// shell's pages end a canvas gesture's trial with the next state (`layout::canvas_live::back`).
pub fn force_next(host: &Arc<Host>) {
    let mut hosts = lock(forced());
    hosts.retain(|weak| weak.strong_count() > 0 && !std::ptr::eq(weak.as_ptr(), &**host));
    hosts.push(Arc::downgrade(host));
}

/// Whether [`force_next`] was asked for `host`'s window since the last time; the forwarder takes
/// it before each push.
pub fn take_forced(host: &Host) -> bool {
    let mut hosts = lock(forced());
    hosts.retain(|weak| weak.strong_count() > 0);
    let before = hosts.len();
    hosts.retain(|weak| !std::ptr::eq(weak.as_ptr(), host));
    hosts.len() != before
}

#[cfg(test)]
#[path = "ipc_push_tests.rs"]
mod tests;
