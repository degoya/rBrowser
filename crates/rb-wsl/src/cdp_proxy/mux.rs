//! Both ends of the tunnel's multiplex. The helper ([`Side::Helper`]) accepts connections on its
//! listener and announces each with `open`; the app ([`Side::App`]) answers an `open` with a
//! connection to its proxy (or `close` when that fails). From then on both ends pump bytes as
//! `data` frames and end a connection with `close` when their local socket ends. A line reader
//! task, a line writer task and one reader and one writer task per connection keep the main
//! loop free of partial reads; per connection at most [`QUEUE`] chunks wait for a slow socket,
//! more ends that connection rather than the memory.

use std::collections::HashMap;
use std::io;
use std::net::SocketAddr;

use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use super::frame::{CHUNK, Frame, decode, encode, payload};
use crate::protocol::MAX_LINE;

/// The most connections at once; more are refused.
pub const MAX_CONNECTIONS: usize = 64;
/// The chunks that may wait for one slow socket.
pub const QUEUE: usize = 64;
/// The frames that may wait for the stdio writer.
const OUTBOX: usize = 256;

/// Which end of the tunnel this is.
#[derive(Debug)]
pub enum Side {
    /// The helper in WSL: accepts on the listener.
    Helper(TcpListener),
    /// The app on Windows: connects to its proxy at the address.
    App(SocketAddr),
}

/// How a multiplex ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ended {
    /// The other end sent `shutdown`.
    Shutdown,
    /// The other end closed its output.
    Closed,
}

struct Connection {
    to_socket: mpsc::Sender<Vec<u8>>,
    reader: JoinHandle<()>,
}

/// Runs the multiplex of `side` with the other end's frames on `input` and ours to `output`,
/// until the other end ends it.
///
/// # Errors
///
/// When reading `input` fails or a line is longer than the protocol allows.
pub async fn run<R, W>(input: R, output: W, side: Side) -> io::Result<Ended>
where
    R: AsyncBufRead + Unpin + Send + 'static,
    W: AsyncWrite + Unpin + Send + 'static,
{
    let (out, out_rx) = mpsc::channel::<Frame>(OUTBOX);
    let writer = tokio::spawn(write_frames(output, out_rx));
    let (lines, mut incoming) = mpsc::channel::<io::Result<String>>(OUTBOX);
    let line_reader = tokio::spawn(read_lines(input, lines));
    let (ended_tx, mut ended) = mpsc::unbounded_channel::<u32>();
    let (listener, proxy) = match side {
        Side::Helper(listener) => (Some(listener), None),
        Side::App(proxy) => (None, Some(proxy)),
    };
    let mut connections: HashMap<u32, Connection> = HashMap::new();
    let mut next_id: u32 = 0;
    let result = loop {
        tokio::select! {
            accepted = accept(listener.as_ref()) => {
                let Ok(stream) = accepted else { continue };
                if connections.len() >= MAX_CONNECTIONS {
                    tracing::warn!("the CDP tunnel refused a connection: too many open");
                    continue;
                }
                next_id = next_free(&connections, next_id);
                let _ = out.send(Frame::Open { id: next_id }).await;
                let connection = spawn_connection(next_id, stream, out.clone(), ended_tx.clone());
                connections.insert(next_id, connection);
            }
            line = incoming.recv() => {
                let frame = match line {
                    None => break Ok(Ended::Closed),
                    Some(Err(error)) => break Err(error),
                    Some(Ok(line)) => match decode(&line) {
                        Ok(frame) => frame,
                        Err(error) => {
                            let _ = out.send(Frame::Error { message: error.to_string() }).await;
                            continue;
                        }
                    },
                };
                match frame {
                    Frame::Open { id } => {
                        let opened = match proxy {
                            Some(proxy) if connections.len() < MAX_CONNECTIONS && !connections.contains_key(&id) => {
                                TcpStream::connect(proxy).await.ok()
                            }
                            _ => None,
                        };
                        match opened {
                            Some(stream) => {
                                let connection = spawn_connection(id, stream, out.clone(), ended_tx.clone());
                                connections.insert(id, connection);
                            }
                            None => {
                                let _ = out.send(Frame::Close { id }).await;
                            }
                        }
                    }
                    Frame::Data { id, data } => {
                        let delivered = match (connections.get(&id), payload(&data)) {
                            (Some(connection), Ok(bytes)) => connection.to_socket.try_send(bytes).is_ok(),
                            _ => false,
                        };
                        if !delivered {
                            close_local(&mut connections, id);
                            let _ = out.send(Frame::Close { id }).await;
                        }
                    }
                    Frame::Close { id } => close_local(&mut connections, id),
                    Frame::Ping => {
                        let _ = out.send(Frame::Pong).await;
                    }
                    Frame::Shutdown => break Ok(Ended::Shutdown),
                    Frame::Pong | Frame::Listening { .. } => {}
                    Frame::Error { message } => {
                        tracing::warn!(%message, "the other end of the CDP tunnel refused a line");
                    }
                }
            }
            Some(id) = ended.recv() => {
                if connections.contains_key(&id) {
                    close_local(&mut connections, id);
                    let _ = out.send(Frame::Close { id }).await;
                }
            }
        }
    };
    for id in connections.keys().copied().collect::<Vec<_>>() {
        close_local(&mut connections, id);
    }
    line_reader.abort();
    drop(out);
    let _ = writer.await;
    result
}

/// The next connection from `listener`; never for the app, which has none.
async fn accept(listener: Option<&TcpListener>) -> io::Result<TcpStream> {
    match listener {
        Some(listener) => listener.accept().await.map(|(stream, _)| stream),
        None => std::future::pending().await,
    }
}

/// An id after `last` that no open connection has.
fn next_free(connections: &HashMap<u32, Connection>, last: u32) -> u32 {
    let mut id = last.wrapping_add(1).max(1);
    while connections.contains_key(&id) {
        id = id.wrapping_add(1).max(1);
    }
    id
}

/// Ends the local side of `id`: its reader stops, its writer sends what waits and closes.
fn close_local(connections: &mut HashMap<u32, Connection>, id: u32) {
    if let Some(connection) = connections.remove(&id) {
        connection.reader.abort();
    }
}

fn spawn_connection(
    id: u32,
    stream: TcpStream,
    out: mpsc::Sender<Frame>,
    ended: mpsc::UnboundedSender<u32>,
) -> Connection {
    let (mut read_half, mut write_half) = stream.into_split();
    let (to_socket, mut from_tunnel) = mpsc::channel::<Vec<u8>>(QUEUE);
    let failed = ended.clone();
    tokio::spawn(async move {
        while let Some(bytes) = from_tunnel.recv().await {
            if write_half.write_all(&bytes).await.is_err() {
                let _ = failed.send(id);
                return;
            }
        }
        let _ = write_half.shutdown().await;
    });
    let reader = tokio::spawn(async move {
        let mut buffer = vec![0u8; CHUNK];
        loop {
            match read_half.read(&mut buffer).await {
                Ok(0) | Err(_) => break,
                Ok(read) => {
                    if out.send(Frame::data(id, &buffer[..read])).await.is_err() {
                        break;
                    }
                }
            }
        }
        let _ = ended.send(id);
    });
    Connection { to_socket, reader }
}

/// Reads lines of at most [`MAX_LINE`] bytes into `lines` until the end or an error.
async fn read_lines<R: AsyncBufRead + Unpin>(
    mut input: R,
    lines: mpsc::Sender<io::Result<String>>,
) {
    loop {
        let mut bytes = Vec::new();
        let limit = u64::try_from(MAX_LINE)
            .unwrap_or(u64::MAX)
            .saturating_add(1);
        let read = (&mut input).take(limit).read_until(b'\n', &mut bytes).await;
        let line = match read {
            Ok(0) => return,
            Ok(_) if bytes.len() > MAX_LINE => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "a CDP tunnel line is too long",
            )),
            Ok(_) => Ok(String::from_utf8_lossy(&bytes).into_owned()),
            Err(error) => Err(error),
        };
        let failed = line.is_err();
        if lines.send(line).await.is_err() || failed {
            return;
        }
    }
}

async fn write_frames<W: AsyncWrite + Unpin>(mut output: W, mut frames: mpsc::Receiver<Frame>) {
    while let Some(frame) = frames.recv().await {
        if output.write_all(encode(&frame).as_bytes()).await.is_err() {
            return;
        }
        // Flush when nothing else waits, so a burst goes out in one write.
        if frames.is_empty() && output.flush().await.is_err() {
            return;
        }
    }
    let _ = output.flush().await;
}

#[cfg(test)]
#[path = "mux_tests.rs"]
mod tests;
