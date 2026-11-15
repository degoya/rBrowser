//! The supervisor's reading of the helper's pipes: the handshake answer, the messages after it
//! (RB-030-05) and the error output kept for a failed start. Split out of `supervisor.rs`.

use std::sync::{Arc, Mutex};

use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, BufReader};
use tokio::process::Child;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::protocol::{HandshakeError, MAX_LINE, Message, check_welcome, decode};

/// Lines before the handshake that are no JSON (a notice of `wsl.exe`) and are skipped.
const MAX_NOISE_LINES: usize = 16;
/// The most bytes of the helper's error output kept for a message.
const STDERR_KEEP: usize = 4096;

/// One line of at most [`MAX_LINE`] bytes; `Ok(None)` at the end of the output.
async fn next_line<R: AsyncRead + Unpin>(
    stdout: &mut BufReader<R>,
) -> Result<Option<String>, HandshakeError> {
    let mut bytes = Vec::new();
    let limit = u64::try_from(MAX_LINE)
        .unwrap_or(u64::MAX)
        .saturating_add(1);
    let read = (&mut *stdout)
        .take(limit)
        .read_until(b'\n', &mut bytes)
        .await
        .map_err(|_| HandshakeError::Closed)?;
    if read == 0 {
        return Ok(None);
    }
    if bytes.len() > MAX_LINE {
        return Err(HandshakeError::TooLong);
    }
    Ok(Some(String::from_utf8_lossy(&bytes).into_owned()))
}

/// The helper's version from its answer: skips up to [`MAX_NOISE_LINES`] lines that are no
/// JSON, then checks the first JSON line.
pub(crate) async fn read_welcome<R: AsyncRead + Unpin>(
    stdout: &mut BufReader<R>,
    nonce: &str,
) -> Result<String, HandshakeError> {
    for _ in 0..=MAX_NOISE_LINES {
        let Some(line) = next_line(stdout).await? else {
            return Err(HandshakeError::Closed);
        };
        if !line.trim_start().starts_with('{') {
            tracing::debug!(line = %line.trim(), "skipped before the WSL helper's handshake");
            continue;
        }
        return check_welcome(&decode(&line)?, nonce);
    }
    Err(HandshakeError::Unexpected(
        "text instead of a handshake".into(),
    ))
}

/// Hands every message the helper writes after its handshake to `sink`, until its output ends
/// or a line is too long. A line that is no message is logged and skipped.
pub(crate) fn forward_messages<R>(
    mut stdout: BufReader<R>,
    sink: mpsc::UnboundedSender<Message>,
) -> JoinHandle<()>
where
    R: AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        loop {
            match next_line(&mut stdout).await {
                Ok(Some(line)) => match decode(&line) {
                    Ok(message) => {
                        if sink.send(message).is_err() {
                            return;
                        }
                    }
                    Err(error) => tracing::debug!(%error, "a line of the WSL helper is skipped"),
                },
                Ok(None) => return,
                Err(error) => {
                    tracing::warn!(%error, "the WSL helper's output is no longer read");
                    return;
                }
            }
        }
    })
}

/// The first [`STDERR_KEEP`] bytes the helper writes to its error output, read in the
/// background and logged line by line.
#[derive(Clone, Default)]
pub(crate) struct KeptStderr(Arc<Mutex<String>>);

impl KeptStderr {
    pub(crate) fn text(&self) -> String {
        let kept = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        kept.lines()
            .map(str::trim)
            .find(|line| !line.is_empty())
            .unwrap_or_default()
            .to_owned()
    }
}

pub(crate) fn keep_stderr(child: &mut Child) -> (KeptStderr, Option<JoinHandle<()>>) {
    let kept = KeptStderr::default();
    let reader = child
        .stderr
        .take()
        .map(|stderr| tokio::spawn(read_stderr(stderr, kept.clone())));
    (kept, reader)
}

/// Reads the helper's error output line by line until it ends: each line logged and kept up to
/// [`STDERR_KEEP`] bytes; a line over [`MAX_LINE`] is skipped, never held whole in memory.
async fn read_stderr<R: AsyncRead + Unpin>(stderr: R, sink: KeptStderr) {
    let mut reader = BufReader::new(stderr);
    let limit = u64::try_from(MAX_LINE)
        .unwrap_or(u64::MAX)
        .saturating_add(1);
    let mut bytes = Vec::new();
    loop {
        bytes.clear();
        match (&mut reader)
            .take(limit)
            .read_until(b'\n', &mut bytes)
            .await
        {
            Ok(0) | Err(_) => return,
            Ok(_) => {}
        }
        if bytes.len() > MAX_LINE {
            tracing::debug!("a WSL helper stderr line over 64 KiB was skipped");
            if !skip_line(&mut reader).await {
                return;
            }
            continue;
        }
        let line = String::from_utf8_lossy(&bytes);
        let line = line.trim_end_matches(['\n', '\r']);
        tracing::debug!(%line, "WSL helper stderr");
        let mut text = sink
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if text.len() < STDERR_KEEP {
            text.push_str(line);
            text.push('\n');
        }
    }
}

/// Reads to the end of the current line in pieces; `false` when the output ended.
async fn skip_line<R: AsyncRead + Unpin>(reader: &mut BufReader<R>) -> bool {
    let mut piece = Vec::new();
    loop {
        piece.clear();
        let limit = u64::try_from(MAX_LINE).unwrap_or(u64::MAX);
        match (&mut *reader)
            .take(limit)
            .read_until(b'\n', &mut piece)
            .await
        {
            Ok(0) | Err(_) => return false,
            Ok(_) if piece.ends_with(b"\n") => return true,
            Ok(_) => {}
        }
    }
}

#[cfg(test)]
#[path = "supervisor_io_tests.rs"]
mod tests;
