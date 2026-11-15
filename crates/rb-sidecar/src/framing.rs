//! Reading lines of bounded length from the sidecar's stdout and stderr: a longer line is skipped
//! up to its newline without being held in memory.

use std::io;

use tokio::io::{AsyncBufRead, AsyncBufReadExt};

/// What [`read_line`] found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineRead {
    /// A line, now in the buffer without its `\n` (and `\r`).
    Line,
    /// A line longer than the limit, skipped; the buffer is empty.
    TooLong,
    /// The end of the stream; a last line without `\n` is dropped.
    Eof,
}

/// Reads the next line into `buf` (cleared first), at most `max` bytes.
pub async fn read_line<R: AsyncBufRead + Unpin>(
    reader: &mut R,
    buf: &mut Vec<u8>,
    max: usize,
) -> io::Result<LineRead> {
    buf.clear();
    let mut too_long = false;
    loop {
        let available = reader.fill_buf().await?;
        if available.is_empty() {
            buf.clear();
            return Ok(LineRead::Eof);
        }
        let newline = available.iter().position(|byte| *byte == b'\n');
        let (part, used) = match newline {
            Some(at) => (&available[..at], at + 1),
            None => (available, available.len()),
        };
        if !too_long {
            if buf.len() + part.len() > max {
                too_long = true;
                buf.clear();
            } else {
                buf.extend_from_slice(part);
            }
        }
        reader.consume(used);
        if newline.is_some() {
            if too_long {
                return Ok(LineRead::TooLong);
            }
            if buf.last() == Some(&b'\r') {
                buf.pop();
            }
            return Ok(LineRead::Line);
        }
    }
}

#[cfg(test)]
#[path = "framing_tests.rs"]
mod tests;
