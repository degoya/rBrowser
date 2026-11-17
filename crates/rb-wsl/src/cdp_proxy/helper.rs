//! The helper's side of `rbrowser-wsl cdp-proxy [--port <port>]`: the handshake of
//! [`crate::protocol`], then a listener on `127.0.0.1:<port>` inside the distribution (a free
//! port when that one is taken or 0 is asked), `listening` with the port, and the multiplex
//! until rBrowser sends `shutdown` or closes the pipe.

use std::io;
use std::net::Ipv4Addr;

use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpListener;

use super::frame::{self, Frame};
use super::mux::{self, Ended, Side};
use crate::protocol::{self, HandshakeError, MAX_LINE, answer_hello};
use crate::serve::Served;

/// Serves the tunnel on `input` and `output` for a helper of version `helper_version`,
/// listening on `port` (0: any).
///
/// # Errors
///
/// When the pipes fail or no port can be opened.
pub async fn serve_cdp_proxy<R, W>(
    mut input: R,
    mut output: W,
    helper_version: &str,
    port: u16,
) -> io::Result<Served>
where
    R: AsyncBufRead + Unpin + Send + 'static,
    W: AsyncWrite + Unpin + Send + 'static,
{
    let mut bytes = Vec::new();
    let limit = u64::try_from(MAX_LINE)
        .unwrap_or(u64::MAX)
        .saturating_add(1);
    let read = (&mut input)
        .take(limit)
        .read_until(b'\n', &mut bytes)
        .await?;
    if read == 0 {
        return Ok(Served::HandshakeFailed(HandshakeError::Closed));
    }
    if bytes.len() > MAX_LINE {
        return Ok(Served::HandshakeFailed(HandshakeError::TooLong));
    }
    let (reply, outcome) = answer_hello(&String::from_utf8_lossy(&bytes), helper_version);
    output
        .write_all(protocol::encode(&reply).as_bytes())
        .await?;
    output.flush().await?;
    if let Err(error) = outcome {
        return Ok(Served::HandshakeFailed(error));
    }
    let listener = listen(port).await?;
    let port = listener.local_addr()?.port();
    output
        .write_all(frame::encode(&Frame::Listening { port }).as_bytes())
        .await?;
    output.flush().await?;
    Ok(
        match mux::run(input, output, Side::Helper(listener)).await? {
            Ended::Shutdown => Served::Shutdown,
            Ended::Closed => Served::Closed,
        },
    )
}

/// A listener on `127.0.0.1:<port>`, or on a free port when that one is taken.
async fn listen(port: u16) -> io::Result<TcpListener> {
    match TcpListener::bind((Ipv4Addr::LOCALHOST, port)).await {
        Ok(listener) => Ok(listener),
        Err(_) if port != 0 => TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await,
        Err(error) => Err(error),
    }
}

#[cfg(test)]
#[path = "helper_tests.rs"]
mod tests;
