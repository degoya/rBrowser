//! The settings writes off the CEF UI thread (RB-080-27). A `setSetting` query arrives on the
//! UI thread (`cef_app/queries.rs` -> `ipc::dispatch_to`), and its answer needs the file: the
//! user file or the project's config, read and written. [`run_in_order`] runs that on a
//! blocking thread of the runtime - one job after the other, in the order they came, so a
//! second change reads what the first wrote and never a stale file. [`reply_in_order`] adds
//! the answer: a job that panics still answers, with an internal failure.

use std::collections::VecDeque;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Mutex, OnceLock};

use rb_ipc::IpcErrorCode;
use tokio::runtime::Handle;

use crate::cdp::lock;
use crate::ipc::Failure;

/// The answer of a job that panicked.
const FAILED: &str = "the setting was not written: the core failed";

type Job = Box<dyn FnOnce() + Send + 'static>;

struct Lane {
    jobs: VecDeque<Job>,
    /// A drainer runs the jobs right now.
    draining: bool,
}

static LANE: OnceLock<Mutex<Lane>> = OnceLock::new();

fn lane() -> &'static Mutex<Lane> {
    LANE.get_or_init(|| {
        Mutex::new(Lane {
            jobs: VecDeque::new(),
            draining: false,
        })
    })
}

/// Runs `job` on a blocking thread of `runtime`, after every job given before it and never
/// beside one. The caller does not wait.
pub fn run_in_order(runtime: &Handle, job: impl FnOnce() + Send + 'static) {
    let start = {
        let mut lane = lock(lane());
        lane.jobs.push_back(Box::new(job));
        !std::mem::replace(&mut lane.draining, true)
    };
    if start {
        let drainer = Drainer { ran: false };
        runtime.spawn_blocking(move || drain(drainer));
    }
}

/// Runs `job` like [`run_in_order`] and hands what it returns to `reply`. A job that panics
/// answers an internal failure, so the shell's query never waits for its IPC timeout.
pub fn reply_in_order<T: Send + 'static>(
    runtime: &Handle,
    job: impl FnOnce() -> T + Send + 'static,
    reply: impl FnOnce(Result<T, Failure>) + Send + 'static,
) {
    run_in_order(runtime, move || {
        let answer = catch_unwind(AssertUnwindSafe(job)).map_err(|_| {
            tracing::warn!("a settings write panicked; the shell gets an error");
            (IpcErrorCode::Internal, FAILED.to_owned())
        });
        reply(answer);
    });
}

/// The one drainer's token. A runtime that is shutting down refuses the blocking task and
/// drops it unrun: the drop frees the lane again, so a later call can start a drainer.
struct Drainer {
    ran: bool,
}

impl Drop for Drainer {
    fn drop(&mut self) {
        if !self.ran {
            lock(lane()).draining = false;
        }
    }
}

/// Runs the queued jobs until none is left. A job that panics is logged by the panic hook and
/// the next one runs: the lane never stalls on it.
fn drain(mut drainer: Drainer) {
    drainer.ran = true;
    loop {
        let job = {
            let mut lane = lock(lane());
            match lane.jobs.pop_front() {
                Some(job) => job,
                None => {
                    lane.draining = false;
                    return;
                }
            }
        };
        if catch_unwind(AssertUnwindSafe(job)).is_err() {
            tracing::warn!("a settings write panicked; the next one runs");
        }
    }
}

#[cfg(test)]
#[path = "lane_tests.rs"]
mod tests;
