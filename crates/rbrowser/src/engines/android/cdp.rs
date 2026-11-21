//! [`CdpClient`]: one CDP connection to Chrome on the device over the forwarded port
//! (RB-095-02). Commands get increasing ids and wait for their answer until [`CALL_TIMEOUT`];
//! a flat session's commands carry its `sessionId`; events go to every subscriber. When the
//! socket ends, waiting commands fail with [`EngineError::Closed`] and the event streams end.
//! Only loopback is dialled (the forward is on 127.0.0.1); frames are untrusted, and one that
//! does not parse is dropped without being quoted.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use rb_engine::EngineError;
use serde_json::{Value, json};
use tokio::net::TcpStream;
use tokio::sync::{broadcast, mpsc, oneshot};
use tokio_tungstenite::tungstenite::Message;

use crate::cdp::lock;

/// How long a command waits for its answer.
pub const CALL_TIMEOUT: Duration = Duration::from_secs(15);
/// How long the connection may take.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// Events a slow subscriber may fall behind before it misses some.
const EVENT_BUFFER: usize = 512;

/// An answer: the result, or CDP's code and message.
type Outcome = Result<Value, (i64, String)>;

/// One CDP event.
#[derive(Clone, Debug, PartialEq)]
pub struct CdpEvent {
    /// `Page.frameNavigated`.
    pub method: String,
    /// Its parameters.
    pub params: Value,
    /// The flat session it belongs to.
    pub session: Option<String>,
}

/// A frame from Chrome.
#[derive(Clone, Debug, PartialEq)]
pub enum Incoming {
    /// The answer to command `id`.
    Answer {
        /// The command's id.
        id: u64,
        /// What it answered.
        outcome: Outcome,
    },
    /// An event.
    Event(CdpEvent),
}

/// The text frame of command `id`.
pub fn command_frame(id: u64, method: &str, params: &Value, session: Option<&str>) -> String {
    let mut frame = json!({ "id": id, "method": method, "params": params });
    if let Some(session) = session {
        frame["sessionId"] = json!(session);
    }
    frame.to_string()
}

/// What a text frame is; `None` when it is neither an answer nor an event.
pub fn parse_incoming(text: &str) -> Option<Incoming> {
    let value: Value = serde_json::from_str(text).ok()?;
    if let Some(id) = value.get("id").and_then(Value::as_u64) {
        let outcome = match value.get("error") {
            Some(error) => Err((
                error
                    .get("code")
                    .and_then(Value::as_i64)
                    .unwrap_or_default(),
                error
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
            )),
            None => Ok(value.get("result").cloned().unwrap_or(Value::Null)),
        };
        return Some(Incoming::Answer { id, outcome });
    }
    let method = value.get("method")?.as_str()?.to_owned();
    Some(Incoming::Event(CdpEvent {
        method,
        params: value.get("params").cloned().unwrap_or(Value::Null),
        session: value
            .get("sessionId")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
    }))
}

/// One CDP connection; clones share it.
#[derive(Clone)]
pub struct CdpClient {
    inner: Arc<Inner>,
}

struct Inner {
    out: mpsc::UnboundedSender<Message>,
    pending: Mutex<HashMap<u64, oneshot::Sender<Outcome>>>,
    next: AtomicU64,
    events: Mutex<Option<broadcast::Sender<CdpEvent>>>,
}

impl CdpClient {
    /// Connects to `url` (`ws://127.0.0.1:<port>/devtools/browser/<id>`).
    ///
    /// # Errors
    ///
    /// Not a loopback `ws://` address, or the connection or handshake failed.
    pub async fn connect(url: &str) -> Result<Self, EngineError> {
        let addr = loopback(url)?;
        let failed = |reason: String| EngineError::Invalid(format!("CDP connection: {reason}"));
        let stream = tokio::time::timeout(CONNECT_TIMEOUT, TcpStream::connect(addr))
            .await
            .map_err(|_| failed("timed out".to_owned()))?
            .map_err(|error| failed(error.kind().to_string()))?;
        let (ws, _) = tokio::time::timeout(
            CONNECT_TIMEOUT,
            tokio_tungstenite::client_async(url, stream),
        )
        .await
        .map_err(|_| failed("the handshake timed out".to_owned()))?
        .map_err(|error| failed(error.to_string()))?;
        let (mut sink, mut stream) = ws.split();
        let (out, mut queue) = mpsc::unbounded_channel::<Message>();
        let (events, _) = broadcast::channel(EVENT_BUFFER);
        let inner = Arc::new(Inner {
            out,
            pending: Mutex::new(HashMap::new()),
            next: AtomicU64::new(1),
            events: Mutex::new(Some(events)),
        });
        tokio::spawn(async move {
            while let Some(message) = queue.recv().await {
                let closing = matches!(message, Message::Close(_));
                if sink.send(message).await.is_err() || closing {
                    break;
                }
            }
            let _closed = sink.close().await;
        });
        let weak: Weak<Inner> = Arc::downgrade(&inner);
        tokio::spawn(async move {
            while let Some(Ok(message)) = stream.next().await {
                let Some(owner) = weak.upgrade() else {
                    return;
                };
                match message {
                    Message::Text(text) => match parse_incoming(text.as_str()) {
                        Some(Incoming::Answer { id, outcome }) => {
                            if let Some(waiting) = lock(&owner.pending).remove(&id) {
                                let _delivered = waiting.send(outcome);
                            }
                        }
                        Some(Incoming::Event(event)) => {
                            if let Some(events) = lock(&owner.events).as_ref() {
                                let _delivered = events.send(event);
                            }
                        }
                        None => tracing::debug!("dropped a CDP frame from the device"),
                    },
                    Message::Close(_) => break,
                    _ => {}
                }
            }
            if let Some(owner) = weak.upgrade() {
                lock(&owner.events).take();
                lock(&owner.pending).clear();
            }
        });
        Ok(Self { inner })
    }

    /// Sends `method` (in `session`, when given) and waits for its answer.
    ///
    /// # Errors
    ///
    /// CDP's refusal, [`EngineError::Closed`] when the connection ended, a timeout.
    pub async fn call(
        &self,
        method: &str,
        params: Value,
        session: Option<&str>,
    ) -> Result<Value, EngineError> {
        if self.is_closed() {
            return Err(EngineError::Closed);
        }
        let id = self.inner.next.fetch_add(1, Ordering::Relaxed);
        let (answer, wait) = oneshot::channel();
        lock(&self.inner.pending).insert(id, answer);
        let frame = command_frame(id, method, &params, session);
        if self.inner.out.send(Message::text(frame)).is_err() {
            lock(&self.inner.pending).remove(&id);
            return Err(EngineError::Closed);
        }
        let outcome = tokio::time::timeout(CALL_TIMEOUT, wait).await;
        lock(&self.inner.pending).remove(&id);
        match outcome {
            Ok(Ok(Ok(result))) => Ok(result),
            Ok(Ok(Err((code, message)))) => Err(EngineError::Cdp {
                method: method.to_owned(),
                code,
                message,
            }),
            Ok(Err(_)) => Err(EngineError::Closed),
            Err(_) => Err(EngineError::Invalid(format!(
                "{method} got no answer within {} s",
                CALL_TIMEOUT.as_secs()
            ))),
        }
    }

    /// The events from now on; ends with the connection.
    pub fn events(&self) -> broadcast::Receiver<CdpEvent> {
        match lock(&self.inner.events).as_ref() {
            Some(events) => events.subscribe(),
            None => broadcast::channel(1).1,
        }
    }

    /// Whether the connection has ended.
    pub fn is_closed(&self) -> bool {
        lock(&self.inner.events).is_none()
    }

    /// Closes the socket.
    pub fn close(&self) {
        let _sent = self.inner.out.send(Message::Close(None));
    }
}

/// The loopback address of a `ws://` URL; [`EngineError::Invalid`] for anything else.
fn loopback(url: &str) -> Result<SocketAddr, EngineError> {
    let invalid = || EngineError::Invalid("the device's socket is not ws://127.0.0.1".to_owned());
    let rest = url.strip_prefix("ws://").ok_or_else(invalid)?;
    let authority = rest.split('/').next().unwrap_or_default();
    let addr: SocketAddr = authority.parse().map_err(|_| invalid())?;
    addr.ip().is_loopback().then_some(addr).ok_or_else(invalid)
}

#[cfg(test)]
#[path = "cdp_tests.rs"]
mod tests;
