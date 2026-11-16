//! The tunnel's messages after the handshake: one JSON object per line, as the helper protocol
//! ([`crate::protocol`]), tagged by `type`. Connection ids are chosen by the helper, which
//! accepts the connections; bytes travel base64-coded in chunks of at most [`CHUNK`] bytes, so
//! every line stays under [`crate::protocol::MAX_LINE`].

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use serde::{Deserialize, Serialize};

use crate::protocol::{HandshakeError, MAX_LINE};

/// The most bytes one `data` frame carries.
pub const CHUNK: usize = 16 * 1024;

/// One tunnel message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum Frame {
    /// Helper to app, once after the handshake: the port it listens on in the distribution.
    Listening {
        /// The port on `127.0.0.1` inside WSL.
        port: u16,
    },
    /// Helper to app: a client connected; the app connects to its proxy.
    Open {
        /// The connection.
        id: u32,
    },
    /// Either way: bytes of a connection.
    Data {
        /// The connection.
        id: u32,
        /// The bytes, base64.
        data: String,
    },
    /// Either way: the connection ended (or was refused).
    Close {
        /// The connection.
        id: u32,
    },
    /// Are you there?
    Ping,
    /// Yes.
    Pong,
    /// App to helper: end.
    Shutdown,
    /// A line that was not understood, for the log.
    Error {
        /// What went wrong.
        message: String,
    },
}

impl Frame {
    /// A `data` frame with `bytes` of connection `id`.
    #[must_use]
    pub fn data(id: u32, bytes: &[u8]) -> Self {
        Self::Data {
            id,
            data: STANDARD.encode(bytes),
        }
    }
}

/// The bytes of a `data` frame's `data`.
///
/// # Errors
///
/// When it is no base64.
pub fn payload(data: &str) -> Result<Vec<u8>, base64::DecodeError> {
    STANDARD.decode(data)
}

/// `frame` as one line, `\n` included.
#[must_use]
pub fn encode(frame: &Frame) -> String {
    let mut line = serde_json::to_string(frame)
        .expect("a frame is strings and numbers, which always serialise");
    line.push('\n');
    line
}

/// The frame of one line.
///
/// # Errors
///
/// [`HandshakeError::TooLong`] past [`MAX_LINE`], [`HandshakeError::Malformed`] for anything
/// that is no frame.
pub fn decode(line: &str) -> Result<Frame, HandshakeError> {
    if line.len() > MAX_LINE {
        return Err(HandshakeError::TooLong);
    }
    serde_json::from_str(line.trim_end())
        .map_err(|error| HandshakeError::Malformed(error.to_string()))
}

#[cfg(test)]
#[path = "frame_tests.rs"]
mod tests;
