//! A transport that hands every frame to a thread of its own, which writes it: a caller on a
//! tokio worker never blocks on a full pipe (RB-094-10). The frames keep their order; after a
//! failed write the thread ends and every later send fails with [`EngineError::Closed`].
//!
//! The queue has no bound - a send never blocks - so the bytes waiting in it are counted
//! ([`rb_util::perf::Gauge`], RB-099-19), and past [`WARN_PENDING_BYTES`] the writer warns once:
//! the browser reads its pipe slower than the core writes.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};

use rb_engine::EngineError;
use rb_util::perf::Gauge;

use super::CdpTransport;

/// The bytes waiting for the writer past which it warns (once per writer).
pub const WARN_PENDING_BYTES: u64 = 32 * 1024 * 1024;

/// The sending end of the writer thread ([`ThreadWriter::spawn`]).
pub struct ThreadWriter {
    frames: mpsc::Sender<Vec<u8>>,
    /// The bytes handed over and not written yet.
    pending: Arc<Gauge>,
    warn_above: u64,
    warned: AtomicBool,
}

impl ThreadWriter {
    /// Starts the thread `name`, which writes each frame with `write` until a write fails or the
    /// writer is dropped. A thread that cannot start leaves a writer whose sends fail.
    pub fn spawn<W>(name: &str, write: W) -> Self
    where
        W: FnMut(&[u8]) -> Result<(), EngineError> + Send + 'static,
    {
        Self::spawn_warning_above(name, WARN_PENDING_BYTES, write)
    }

    /// [`ThreadWriter::spawn`] that warns past `warn_above` waiting bytes.
    fn spawn_warning_above<W>(name: &str, warn_above: u64, mut write: W) -> Self
    where
        W: FnMut(&[u8]) -> Result<(), EngineError> + Send + 'static,
    {
        let (frames, inbox) = mpsc::channel::<Vec<u8>>();
        let pending = Arc::new(Gauge::new());
        let written = Arc::clone(&pending);
        let spawned = std::thread::Builder::new()
            .name(name.to_owned())
            .spawn(move || {
                while let Ok(frame) = inbox.recv() {
                    let result = write(&frame);
                    written.sub(byte_len(&frame));
                    if let Err(error) = result {
                        tracing::warn!(%error, "a CDP message was not written; the writer stops");
                        // The frames still queued go with the inbox, and every later send
                        // fails: nothing waits any more.
                        drop(inbox);
                        written.reset();
                        return;
                    }
                }
            });
        if let Err(error) = spawned {
            tracing::error!(%error, "the CDP writer could not start");
        }
        Self {
            frames,
            pending,
            warn_above,
            warned: AtomicBool::new(false),
        }
    }

    /// The bytes handed over and not written yet.
    pub fn pending_bytes(&self) -> u64 {
        self.pending.now()
    }

    /// Whether the writer warned that it falls behind (tests).
    #[cfg(test)]
    fn warned(&self) -> bool {
        self.warned.load(Ordering::Relaxed)
    }
}

impl CdpTransport for ThreadWriter {
    fn send(&self, frame: Vec<u8>) -> Result<(), EngineError> {
        let bytes = byte_len(&frame);
        let waiting = self.pending.add(bytes);
        if waiting > self.warn_above && !self.warned.swap(true, Ordering::Relaxed) {
            tracing::warn!(
                waiting,
                peak = self.pending.peak(),
                "the CDP writer falls behind: the browser reads its pipe slower than the core writes"
            );
        }
        self.frames.send(frame).map_err(|_| {
            self.pending.sub(bytes);
            EngineError::Closed
        })
    }
}

fn byte_len(frame: &[u8]) -> u64 {
    u64::try_from(frame.len()).unwrap_or(u64::MAX)
}

#[cfg(test)]
#[path = "writer_tests.rs"]
mod tests;
