//! The helper's side of the protocol: the handshake, then `ping` and `shutdown` until rBrowser
//! closes the pipe. Kept apart from `src/bin/rbrowser-wsl.rs` so it runs in tests over
//! in-memory pipes. `watch` has its own loop on the same handshake (`crate::watch::run`,
//! RB-030-05); `cdp-proxy` adds its messages with RB-030-06.

use std::io::{self, BufRead, Read as _, Write};

use crate::protocol::{ErrorCode, HandshakeError, MAX_LINE, Message, answer_hello, decode, encode};

/// How a helper run ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Served {
    /// rBrowser sent `shutdown`.
    Shutdown,
    /// rBrowser closed the pipe (it ended or killed the helper).
    Closed,
    /// The handshake failed; the reply has been written.
    HandshakeFailed(HandshakeError),
}

/// Serves `input` into `output` for a helper of version `helper_version`.
///
/// # Errors
///
/// When reading or writing the pipes fails.
pub fn serve<R: BufRead, W: Write>(
    mut input: R,
    mut output: W,
    helper_version: &str,
) -> io::Result<Served> {
    if let Some(failed) = handshake(&mut input, &mut output, helper_version)? {
        return Ok(failed);
    }
    loop {
        let line = match read_line(&mut input, MAX_LINE)? {
            Line::Text(line) => line,
            Line::End => return Ok(Served::Closed),
            Line::TooLong => {
                skip_line(&mut input)?;
                send(&mut output, &bad_request("a line is too long"))?;
                continue;
            }
        };
        match decode(&line) {
            Ok(Message::Ping) => send(&mut output, &Message::Pong)?,
            Ok(Message::Shutdown) => return Ok(Served::Shutdown),
            Ok(other) => send(
                &mut output,
                &bad_request(&format!("unexpected {}", other.kind())),
            )?,
            Err(error) => send(&mut output, &bad_request(&error.to_string()))?,
        }
    }
}

/// Answers the first line of `input`: `None` after a good handshake, else how the run ended (the
/// reply is written either way).
///
/// # Errors
///
/// When reading or writing the pipes fails.
pub fn handshake<R: BufRead, W: Write>(
    input: &mut R,
    output: &mut W,
    helper_version: &str,
) -> io::Result<Option<Served>> {
    let first = match read_line(input, MAX_LINE)? {
        Line::Text(line) => line,
        Line::End => return Ok(Some(Served::HandshakeFailed(HandshakeError::Closed))),
        Line::TooLong => return Ok(Some(Served::HandshakeFailed(HandshakeError::TooLong))),
    };
    let (reply, outcome) = answer_hello(&first, helper_version);
    send(output, &reply)?;
    Ok(outcome.err().map(Served::HandshakeFailed))
}

/// One line read by [`read_line`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Line {
    /// The line, its `\n` included; invalid UTF-8 replaced.
    Text(String),
    /// Longer than the limit; `max + 1` bytes of it are read, the rest is not.
    TooLong,
    /// The end of the input.
    End,
}

/// One line of at most `max` bytes.
///
/// # Errors
///
/// The reader's error.
pub fn read_line<R: BufRead>(input: &mut R, max: usize) -> io::Result<Line> {
    let mut bytes = Vec::new();
    let limit = u64::try_from(max).unwrap_or(u64::MAX).saturating_add(1);
    input.by_ref().take(limit).read_until(b'\n', &mut bytes)?;
    if bytes.is_empty() {
        return Ok(Line::End);
    }
    if bytes.len() > max {
        return Ok(Line::TooLong);
    }
    Ok(Line::Text(String::from_utf8_lossy(&bytes).into_owned()))
}

/// Reads past the rest of an overlong line.
pub(crate) fn skip_line<R: BufRead>(input: &mut R) -> io::Result<()> {
    loop {
        let buffer = input.fill_buf()?;
        if buffer.is_empty() {
            return Ok(());
        }
        if let Some(end) = buffer.iter().position(|byte| *byte == b'\n') {
            input.consume(end + 1);
            return Ok(());
        }
        let len = buffer.len();
        input.consume(len);
    }
}

/// An `error` with code `bad_request`.
pub(crate) fn bad_request(message: &str) -> Message {
    Message::Error {
        code: ErrorCode::BadRequest,
        message: message.to_owned(),
        protocol: None,
    }
}

/// Writes `message` as one line and flushes it.
pub(crate) fn send<W: Write>(output: &mut W, message: &Message) -> io::Result<()> {
    output.write_all(encode(message).as_bytes())?;
    output.flush()
}

#[cfg(test)]
#[path = "serve_tests.rs"]
mod tests;
