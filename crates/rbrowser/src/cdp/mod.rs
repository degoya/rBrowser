//! The app's own CDP client on the pipe (ADR 0001, Decision 4): CEF exposes CDP only over the
//! in-process pipes of `--remote-debugging-io-pipes`; messages are JSON, each ended by a NUL
//! byte. One [`CdpRouter`] multiplexes every caller over the one pipe (flat sessions); the
//! external token proxy (RB-030-06) will share it. The pipe itself is Windows code
//! (`crate::cef_app::pipe`); everything here runs anywhere.

#[cfg(test)]
pub(crate) mod fake;
mod frame;
mod router;
mod writer;

pub use frame::{FrameDecoder, encode};
pub(crate) use router::lock;
pub use router::{CdpEvent, CdpRouter, CdpTransport, Tap};
pub use writer::ThreadWriter;
