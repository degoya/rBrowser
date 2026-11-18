//! The setup wizard's steps off the UI thread (RB-091-05): `preview`, `write` and `basics` run
//! on the runtime's blocking pool - the analyser reads a site, the files may lie in WSL - one
//! after the other in the order they came, never beside each other. So a proposal is kept before
//! the next preview starts, and a write takes the proposal the page saw last. A step that panics
//! is logged and the next one runs. Its own lane, so a slow analysis never holds up the settings
//! writes (`settings_window::lane`).

use std::collections::VecDeque;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Mutex;

use tokio::runtime::Handle;

use crate::cdp::lock;

type Job = Box<dyn FnOnce() + Send + 'static>;

struct Lane {
    jobs: VecDeque<Job>,
    /// A drainer runs the jobs right now.
    draining: bool,
}

static LANE: Mutex<Lane> = Mutex::new(Lane {
    jobs: VecDeque::new(),
    draining: false,
});

/// Runs `job` on a blocking thread of `runtime`, after every job given before it and never
/// beside one. The caller does not wait.
pub fn run_in_order(runtime: &Handle, job: impl FnOnce() + Send + 'static) {
    let start = {
        let mut lane = lock(&LANE);
        lane.jobs.push_back(Box::new(job));
        !std::mem::replace(&mut lane.draining, true)
    };
    if start {
        let drainer = Drainer { ran: false };
        runtime.spawn_blocking(move || drain(drainer));
    }
}

/// The one drainer's token. A runtime that is shutting down drops the blocking task unrun: the
/// drop frees the lane again, so a later call can start a drainer.
struct Drainer {
    ran: bool,
}

impl Drop for Drainer {
    fn drop(&mut self) {
        if !self.ran {
            lock(&LANE).draining = false;
        }
    }
}

fn drain(mut drainer: Drainer) {
    drainer.ran = true;
    loop {
        let job = {
            let mut lane = lock(&LANE);
            match lane.jobs.pop_front() {
                Some(job) => job,
                None => {
                    lane.draining = false;
                    return;
                }
            }
        };
        if catch_unwind(AssertUnwindSafe(job)).is_err() {
            tracing::warn!("a step of the setup wizard panicked; the next one runs");
        }
    }
}

#[cfg(test)]
#[path = "lane_tests.rs"]
mod tests;
