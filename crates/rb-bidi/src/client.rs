//! [`BidiClient`]: one WebDriver BiDi connection. Commands get increasing ids and wait for the
//! answer with that id (in any order) until their timeout; events go to every subscriber. When
//! the socket ends, every waiting command fails with [`BidiError::Closed`], the event streams
//! end and [`BidiClient::closed`] resolves.
//!
//! Only loopback endpoints are dialled ([`loopback_target`]): Firefox prints its endpoint on
//! stderr, and nothing else is ever taken for one. Frames are untrusted: one that does not parse
//! is dropped with a debug line that never quotes it.

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;
use tokio::sync::{broadcast, mpsc, oneshot, watch};
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::Message;

use crate::error::BidiError;
use crate::protocol::{Command, Event, Failure, Incoming, command_frame, parse_incoming};

/// How long a command waits for its answer unless it says otherwise.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);
/// Events a slow subscriber may fall behind before it misses some.
const EVENT_BUFFER: usize = 1024;

type Answer = Result<Value, Failure>;

/// One BiDi connection; clones share it.
#[derive(Clone)]
pub struct BidiClient {
    inner: Arc<Inner>,
}

struct Inner {
    out: mpsc::UnboundedSender<Message>,
    pending: Mutex<HashMap<u64, oneshot::Sender<Answer>>>,
    next: AtomicU64,
    events: Mutex<Option<broadcast::Sender<Event>>>,
    closed: watch::Sender<bool>,
    timeout: Duration,
}

impl BidiClient {
    /// Connects to `endpoint` (`ws://127.0.0.1:<port>/session`); refuses anything that is not
    /// loopback. `timeout` bounds the connect and is every command's default.
    pub async fn connect(endpoint: &str, timeout: Duration) -> Result<Self, BidiError> {
        let (addr, url) = loopback_target(endpoint)?;
        let stream = tokio::time::timeout(timeout, TcpStream::connect(addr))
            .await
            .map_err(|_| BidiError::Connect("timed out".to_owned()))?
            .map_err(|error| BidiError::Connect(error.kind().to_string()))?;
        let (ws, _response) = tokio::time::timeout(
            timeout,
            tokio_tungstenite::client_async(url.as_str(), stream),
        )
        .await
        .map_err(|_| BidiError::Connect("the handshake timed out".to_owned()))?
        .map_err(|error| BidiError::Connect(error.to_string()))?;
        Ok(Self::over(ws, timeout))
    }

    /// A client over an open WebSocket; `timeout` is every command's default.
    pub fn over<S>(ws: WebSocketStream<S>, timeout: Duration) -> Self
    where
        S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    {
        let (sink, stream) = ws.split();
        let (out, queue) = mpsc::unbounded_channel();
        let (events, _) = broadcast::channel(EVENT_BUFFER);
        let (closed, _) = watch::channel(false);
        let inner = Arc::new(Inner {
            out,
            pending: Mutex::new(HashMap::new()),
            next: AtomicU64::new(1),
            events: Mutex::new(Some(events)),
            closed,
            timeout,
        });
        tokio::spawn(write(sink, queue));
        tokio::spawn(read(stream, Arc::downgrade(&inner)));
        Self { inner }
    }

    /// Sends `method` and waits for its answer within the default timeout.
    pub async fn command(&self, method: &str, params: Value) -> Result<Value, BidiError> {
        self.command_within(method, params, self.inner.timeout)
            .await
    }

    /// Sends `command` and waits for its answer within the default timeout.
    pub async fn send(&self, command: Command) -> Result<Value, BidiError> {
        self.command(command.method, command.params).await
    }

    /// Sends `command` and waits for its answer within `timeout`.
    pub async fn send_within(
        &self,
        command: Command,
        timeout: Duration,
    ) -> Result<Value, BidiError> {
        self.command_within(command.method, command.params, timeout)
            .await
    }

    /// Sends `method` and waits for its answer within `timeout`. A late answer is dropped.
    pub async fn command_within(
        &self,
        method: &str,
        params: Value,
        timeout: Duration,
    ) -> Result<Value, BidiError> {
        if self.is_closed() {
            return Err(BidiError::Closed);
        }
        let id = self.inner.next.fetch_add(1, Ordering::Relaxed);
        let (answer, wait) = oneshot::channel();
        lock(&self.inner.pending).insert(id, answer);
        let _forget = Forget {
            inner: &self.inner,
            id,
        };
        // The end may have come between the check above and the insert.
        if self.is_closed() {
            return Err(BidiError::Closed);
        }
        let frame = command_frame(id, method, params);
        if self.inner.out.send(Message::text(frame)).is_err() {
            return Err(BidiError::Closed);
        }
        match tokio::time::timeout(timeout, wait).await {
            Ok(Ok(Ok(result))) => Ok(result),
            Ok(Ok(Err(failure))) => Err(BidiError::refused(method, failure)),
            Ok(Err(_)) => Err(BidiError::Closed),
            Err(_) => Err(BidiError::Timeout {
                method: method.to_owned(),
                ms: u64::try_from(timeout.as_millis()).unwrap_or(u64::MAX),
            }),
        }
    }

    /// The events from now on; ends when the connection does.
    pub fn events(&self) -> broadcast::Receiver<Event> {
        match lock(&self.inner.events).as_ref() {
            Some(events) => events.subscribe(),
            None => broadcast::channel(1).1,
        }
    }

    /// Whether the connection has ended.
    pub fn is_closed(&self) -> bool {
        *self.inner.closed.borrow()
    }

    /// Resolves once the connection has ended.
    pub async fn closed(&self) {
        let mut closed = self.inner.closed.subscribe();
        // `wait_for` fails only when the sender is gone, which is the end as well.
        let _ended = closed.wait_for(|closed| *closed).await;
    }

    /// Closes the socket; waiting commands fail with [`BidiError::Closed`].
    pub fn close(&self) {
        // A closed queue means the writer has already ended.
        let _sent = self.inner.out.send(Message::Close(None));
    }
}

/// Forgets a command's answer slot when its caller stops waiting (answered, timed out, dropped).
struct Forget<'a> {
    inner: &'a Inner,
    id: u64,
}

impl Drop for Forget<'_> {
    fn drop(&mut self) {
        lock(&self.inner.pending).remove(&self.id);
    }
}

impl Inner {
    fn answer(&self, id: u64, outcome: Answer) {
        if let Some(waiting) = lock(&self.pending).remove(&id) {
            // The caller may have stopped waiting.
            let _delivered = waiting.send(outcome);
        }
    }

    fn event(&self, event: Event) {
        if let Some(events) = lock(&self.events).as_ref() {
            // No subscriber is fine.
            let _delivered = events.send(event);
        }
    }

    /// Closed first, then the waiting commands fail: a command that registers in between sees
    /// the flag.
    fn end(&self) {
        self.closed.send_replace(true);
        lock(&self.events).take();
        lock(&self.pending).clear();
    }
}

type Sink<S> = SplitSink<WebSocketStream<S>, Message>;

async fn write<S>(mut sink: Sink<S>, mut queue: mpsc::UnboundedReceiver<Message>)
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    while let Some(message) = queue.recv().await {
        let closing = matches!(message, Message::Close(_));
        if sink.send(message).await.is_err() || closing {
            break;
        }
    }
    // The other side may be gone already.
    let _closed = sink.close().await;
}

async fn read<S>(mut stream: SplitStream<WebSocketStream<S>>, inner: Weak<Inner>)
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    while let Some(message) = stream.next().await {
        let Some(owner) = inner.upgrade() else {
            return;
        };
        match message {
            Ok(Message::Text(text)) => match parse_incoming(text.as_str()) {
                Ok(Incoming::Answer { id, outcome }) => owner.answer(id, outcome),
                Ok(Incoming::Event(event)) => owner.event(event),
                Ok(Incoming::Unrouted(failure)) => {
                    tracing::debug!(error = %failure.error, "Firefox could not route a command");
                }
                Err(reason) => tracing::debug!(%reason, "dropped a BiDi frame"),
            },
            Ok(Message::Close(_)) | Err(_) => break,
            Ok(_) => {}
        }
    }
    if let Some(owner) = inner.upgrade() {
        owner.end();
    }
}

/// The socket address and URL of a `ws://` endpoint on this machine; [`BidiError::NotLoopback`]
/// for any other host, [`BidiError::Connect`] for a URL that is not `ws://host:port[/path]`.
pub fn loopback_target(endpoint: &str) -> Result<(SocketAddr, String), BidiError> {
    let invalid = || BidiError::Connect("the endpoint is not ws://host:port".to_owned());
    let rest = endpoint.strip_prefix("ws://").ok_or_else(invalid)?;
    let authority = rest.split('/').next().unwrap_or_default();
    let (host, port) = authority.rsplit_once(':').ok_or_else(invalid)?;
    let port: u16 = port.parse().map_err(|_| invalid())?;
    let ip: IpAddr = match host {
        "localhost" => IpAddr::V4(Ipv4Addr::LOCALHOST),
        bracketed if bracketed.starts_with('[') && bracketed.ends_with(']') => bracketed
            [1..bracketed.len() - 1]
            .parse()
            .map_err(|_| BidiError::NotLoopback)?,
        plain => plain.parse().map_err(|_| BidiError::NotLoopback)?,
    };
    if !ip.is_loopback() {
        return Err(BidiError::NotLoopback);
    }
    Ok((SocketAddr::new(ip, port), endpoint.to_owned()))
}

pub(crate) use rb_util::lock;

#[cfg(test)]
#[path = "client_tests.rs"]
mod tests;
