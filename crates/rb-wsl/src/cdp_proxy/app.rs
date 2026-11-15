//! The app's side of the tunnel: start `rbrowser-wsl cdp-proxy` through `wsl.exe` (the command
//! from [`crate::helper::helper_args`] and [`crate::query::wsl_command`]), do the handshake,
//! read the port the helper listens on, and run the multiplex toward the proxy on Windows'
//! `127.0.0.1` until the helper or the app ends it. Dropping the [`Tunnel`] stops both: the
//! task is aborted and the helper's process killed.

use std::io;
use std::net::SocketAddr;
use std::process::Stdio;
use std::time::Duration;

use rb_process::NoConsoleWindow;
use tokio::io::{
    AsyncBufRead, AsyncBufReadExt, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader,
};
use tokio::process::{Child, Command};
use tokio::task::JoinHandle;

use super::frame::{self, Frame};
use super::mux::{self, Ended, Side};
use crate::WslError;
use crate::protocol::{self, HandshakeError, MAX_LINE, check_welcome, hello, new_nonce};

/// Lines before the handshake that are no JSON (a notice of `wsl.exe`) and are skipped.
const MAX_NOISE_LINES: usize = 16;

/// A running tunnel.
#[derive(Debug)]
pub struct Tunnel {
    port: u16,
    task: JoinHandle<io::Result<Ended>>,
    _child: Child,
}

impl Tunnel {
    /// The port the helper listens on inside the distribution.
    #[must_use]
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Whether the multiplex still runs.
    #[must_use]
    pub fn is_running(&self) -> bool {
        !self.task.is_finished()
    }

    /// Waits until the tunnel ends (the helper ended or was ended).
    ///
    /// # Errors
    ///
    /// When the multiplex failed.
    pub async fn wait(mut self) -> io::Result<Ended> {
        (&mut self.task)
            .await
            .map_err(|error| io::Error::other(error.to_string()))?
    }
}

impl Drop for Tunnel {
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// Starts the helper with `command` (`wsl.exe -d <distro> -e <helper> cdp-proxy --port <n>`) and
/// tunnels its connections to `proxy`. The handshake and the `listening` line may take `limit`
/// (a stopped distribution boots within it).
///
/// # Errors
///
/// [`WslError::NotInstalled`] without `wsl.exe`, [`WslError::Handshake`] for a helper from
/// another package, [`WslError::HelperStart`] for anything else that went wrong.
pub async fn start(
    mut command: Command,
    proxy: SocketAddr,
    limit: Duration,
) -> Result<Tunnel, WslError> {
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .no_console_window();
    let mut child = command.spawn().map_err(|error| {
        if error.kind() == io::ErrorKind::NotFound {
            WslError::NotInstalled
        } else {
            WslError::HelperStart(error.to_string())
        }
    })?;
    let (Some(mut stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else {
        return Err(WslError::HelperStart(
            "the helper's pipes are missing".into(),
        ));
    };
    let mut stdout = BufReader::new(stdout);
    let port = match tokio::time::timeout(limit, handshake(&mut stdout, &mut stdin)).await {
        Ok(Ok(port)) => port,
        Ok(Err(error)) => return Err(error),
        Err(_) => {
            return Err(WslError::Timeout(
                u64::try_from(limit.as_millis()).unwrap_or(u64::MAX),
            ));
        }
    };
    let task = tokio::spawn(mux::run(stdout, stdin, Side::App(proxy)));
    Ok(Tunnel {
        port,
        task,
        _child: child,
    })
}

/// The app's handshake on the helper's pipes: `hello`, the `welcome` (lines that are no JSON
/// skipped), then the `listening` port.
///
/// # Errors
///
/// [`WslError::Handshake`] for a refused or broken handshake, [`WslError::HelperStart`] when
/// no `listening` follows.
pub async fn handshake<R, W>(reader: &mut R, writer: &mut W) -> Result<u16, WslError>
where
    R: AsyncBufRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let nonce = new_nonce();
    writer
        .write_all(protocol::encode(&hello(&nonce)).as_bytes())
        .await
        .map_err(|_| HandshakeError::Closed)?;
    writer.flush().await.map_err(|_| HandshakeError::Closed)?;
    let mut welcomed = false;
    for _ in 0..=MAX_NOISE_LINES {
        let line = read_line(reader).await?;
        if !line.trim_start().starts_with('{') {
            continue;
        }
        if !welcomed {
            check_welcome(&protocol::decode(&line)?, &nonce)?;
            welcomed = true;
            continue;
        }
        return match frame::decode(&line)? {
            Frame::Listening { port } => Ok(port),
            other => Err(WslError::HelperStart(format!(
                "the helper answered {other:?} instead of its port"
            ))),
        };
    }
    Err(HandshakeError::Unexpected("text instead of a handshake".into()).into())
}

async fn read_line<R: AsyncBufRead + Unpin>(reader: &mut R) -> Result<String, HandshakeError> {
    let mut bytes = Vec::new();
    let limit = u64::try_from(MAX_LINE)
        .unwrap_or(u64::MAX)
        .saturating_add(1);
    let read = reader
        .take(limit)
        .read_until(b'\n', &mut bytes)
        .await
        .map_err(|_| HandshakeError::Closed)?;
    if read == 0 {
        return Err(HandshakeError::Closed);
    }
    if bytes.len() > MAX_LINE {
        return Err(HandshakeError::TooLong);
    }
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

#[cfg(test)]
#[path = "app_tests.rs"]
mod tests;
