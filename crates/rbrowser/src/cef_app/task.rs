//! Running work on the CEF UI thread, where every Views call must happen.

use std::sync::{Arc, Mutex};

use cef::*;

use crate::cdp::lock;

type Job = Box<dyn FnOnce() + Send>;

wrap_task! {
    struct FnTask {
        job: Arc<Mutex<Option<Job>>>,
    }

    impl Task {
        fn execute(&self) {
            let job = lock(&self.job).take();
            if let Some(job) = job {
                job();
            }
        }
    }
}

/// Queues `job` on the CEF UI thread; returns at once.
pub fn on_ui(job: impl FnOnce() + Send + 'static) {
    let mut task = FnTask::new(Arc::new(Mutex::new(Some(Box::new(job)))));
    if post_task(ThreadId::UI, Some(&mut task)) == 0 {
        tracing::warn!("the CEF UI thread is gone; a window task was dropped");
    }
}

/// Queues `job` on the CEF UI thread after `delay_ms`; returns at once (the overlay of a cell
/// label, RB-010-16).
pub fn on_ui_after(delay_ms: u32, job: impl FnOnce() + Send + 'static) {
    let mut task = FnTask::new(Arc::new(Mutex::new(Some(Box::new(job)))));
    if post_delayed_task(ThreadId::UI, Some(&mut task), i64::from(delay_ms)) == 0 {
        tracing::warn!("the CEF UI thread is gone; a delayed window task was dropped");
    }
}
