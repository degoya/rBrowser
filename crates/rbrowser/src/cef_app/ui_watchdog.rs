//! A watch on the CEF UI thread: once a second a task is queued there and the time it waits is
//! measured; a long wait goes into the log with its length (`crate::perf::ui_stall`), and a
//! thread that does not answer for [`STUCK`] is logged while it is still stuck. Every window call runs on that
//! thread, so a long wait is a frozen window - the log then says when and for how long.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use tokio::runtime::Handle;
use tokio::sync::oneshot;

use super::task::on_ui;
use crate::perf::ui_stall::{STUCK, verdict};

/// The time between two probes.
const EVERY: Duration = Duration::from_secs(1);

static STOPPED: AtomicBool = AtomicBool::new(false);

/// Starts the watch on `runtime`; it ends with [`stop`].
pub fn start(runtime: &Handle) {
    STOPPED.store(false, Ordering::Release);
    runtime.spawn(async {
        while !STOPPED.load(Ordering::Acquire) {
            tokio::time::sleep(EVERY).await;
            let (answer, answered) = oneshot::channel();
            let sent = Instant::now();
            on_ui(move || {
                let _ = answer.send(());
            });
            let mut answered = std::pin::pin!(answered);
            if tokio::time::timeout(STUCK, &mut answered).await.is_err() {
                tracing::warn!(
                    secs = STUCK.as_secs(),
                    "the UI thread has not answered; the window is frozen"
                );
                if answered.await.is_err() {
                    return;
                }
            }
            let waited = sent.elapsed();
            if let Some(message) = verdict(waited) {
                let ms = u64::try_from(waited.as_millis()).unwrap_or(u64::MAX);
                tracing::warn!(ms, "{message}");
            }
        }
    });
}

/// Ends the watch (the window closes).
pub fn stop() {
    STOPPED.store(true, Ordering::Release);
}
