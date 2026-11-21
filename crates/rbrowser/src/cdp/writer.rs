//! A transport that hands every frame to a thread of its own, which writes it: a caller on a
//! tokio worker never blocks on a full pipe (RB-094-10). The frames keep their order; after a
//! failed write the thread ends and every later send fails with [`EngineError::Closed`].

use std::sync::mpsc;

use rb_engine::EngineError;

use super::CdpTransport;

/// The sending end of the writer thread ([`ThreadWriter::spawn`]).
pub struct ThreadWriter {
    frames: mpsc::Sender<Vec<u8>>,
}

impl ThreadWriter {
    /// Starts the thread `name`, which writes each frame with `write` until a write fails or the
    /// writer is dropped. A thread that cannot start leaves a writer whose sends fail.
    pub fn spawn<W>(name: &str, mut write: W) -> Self
    where
        W: FnMut(&[u8]) -> Result<(), EngineError> + Send + 'static,
    {
        let (frames, inbox) = mpsc::channel::<Vec<u8>>();
        let spawned = std::thread::Builder::new()
            .name(name.to_owned())
            .spawn(move || {
                for frame in inbox {
                    if let Err(error) = write(&frame) {
                        tracing::warn!(%error, "a CDP message was not written; the writer stops");
                        return;
                    }
                }
            });
        if let Err(error) = spawned {
            tracing::error!(%error, "the CDP writer could not start");
        }
        Self { frames }
    }
}

impl CdpTransport for ThreadWriter {
    fn send(&self, frame: Vec<u8>) -> Result<(), EngineError> {
        self.frames.send(frame).map_err(|_| EngineError::Closed)
    }
}

#[cfg(test)]
#[path = "writer_tests.rs"]
mod tests;
