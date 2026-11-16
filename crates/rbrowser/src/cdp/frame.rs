//! The pipe's framing: a JSON message followed by a NUL byte.

use serde_json::Value;

/// Collects the bytes read from the pipe and cuts them into messages.
#[derive(Debug, Default)]
pub struct FrameDecoder {
    pending: Vec<u8>,
}

impl FrameDecoder {
    /// Adds `bytes` and returns every message completed by them, without the NUL.
    pub fn push(&mut self, bytes: &[u8]) -> Vec<Vec<u8>> {
        let mut messages = Vec::new();
        for chunk in bytes.split_inclusive(|byte| *byte == 0) {
            if let Some((0, message)) = chunk.split_last() {
                self.pending.extend_from_slice(message);
                messages.push(std::mem::take(&mut self.pending));
            } else {
                self.pending.extend_from_slice(chunk);
            }
        }
        messages
    }

    /// Bytes of a message not completed yet.
    pub fn pending(&self) -> usize {
        self.pending.len()
    }
}

/// The bytes that carry `message` over the pipe.
pub fn encode(message: &Value) -> Result<Vec<u8>, serde_json::Error> {
    let mut bytes = serde_json::to_vec(message)?;
    bytes.push(0);
    Ok(bytes)
}

#[cfg(test)]
#[path = "frame_tests.rs"]
mod tests;
