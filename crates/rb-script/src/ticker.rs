//! The epoch ticker of the script engine, as rb-plugin-host's (`engine.rs`, RB-090-38): the
//! epoch moves every [`EPOCH_TICK`] while a run's store lives, and the thread is parked while
//! none does.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread::JoinHandle;
use std::time::Duration;

use wasmtime::Engine;

/// How often the epoch moves while a script runs: each tick the running script yields to the
/// executor and its deadline is looked at.
pub const EPOCH_TICK: Duration = Duration::from_millis(10);

/// What the ticker thread and the running stores share.
#[derive(Default)]
struct State {
    active: AtomicUsize,
    stop: AtomicBool,
}

/// The ticker of one engine.
pub(crate) struct Ticker {
    state: Arc<State>,
    thread: Option<JoinHandle<()>>,
}

/// Keeps the epoch moving while held: one per run.
pub(crate) struct Ticking {
    state: Arc<State>,
}

impl Drop for Ticking {
    fn drop(&mut self) {
        self.state.active.fetch_sub(1, Ordering::AcqRel);
    }
}

impl Ticker {
    pub(crate) fn new(engine: Engine) -> Self {
        let state = Arc::new(State::default());
        let shared = Arc::clone(&state);
        let thread = std::thread::spawn(move || {
            while !shared.stop.load(Ordering::Acquire) {
                if shared.active.load(Ordering::Acquire) == 0 {
                    std::thread::park();
                    continue;
                }
                std::thread::sleep(EPOCH_TICK);
                engine.increment_epoch();
            }
        });
        Self {
            state,
            thread: Some(thread),
        }
    }

    /// The epoch moves while the answer is held; the first holder wakes the thread.
    pub(crate) fn ticking(&self) -> Ticking {
        if self.state.active.fetch_add(1, Ordering::AcqRel) == 0
            && let Some(thread) = &self.thread
        {
            thread.thread().unpark();
        }
        Ticking {
            state: Arc::clone(&self.state),
        }
    }
}

impl Drop for Ticker {
    fn drop(&mut self) {
        self.state.stop.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            thread.thread().unpark();
            if thread.join().is_err() {
                tracing::warn!("the script engine's epoch ticker panicked");
            }
        }
    }
}
