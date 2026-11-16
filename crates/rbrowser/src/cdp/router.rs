//! Request/response matching and event fan-out over one CDP connection.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rb_engine::EngineError;
use serde_json::{Value, json};
use tokio::sync::{broadcast, mpsc, oneshot};

use super::frame::encode;

/// How long a command may take before its caller gets an error.
const CALL_TIMEOUT: Duration = Duration::from_secs(30);
/// Events kept for a subscriber that falls behind.
const EVENT_BUFFER: usize = 1024;

/// Where encoded messages go: the pipe on Windows, a recorder in the tests.
pub trait CdpTransport: Send + Sync {
    /// Writes one encoded message (JSON + NUL).
    fn send(&self, frame: Vec<u8>) -> Result<(), EngineError>;
}

/// A CDP event, with the flat session it came from (`None` for the browser target).
#[derive(Clone, Debug, PartialEq)]
pub struct CdpEvent {
    /// The method (`Target.detachedFromTarget`).
    pub method: String,
    /// Its parameters.
    pub params: Value,
    /// The session, for events of an attached target.
    pub session_id: Option<String>,
}

type Reply = Result<Value, EngineError>;

struct Pending {
    method: String,
    reply: oneshot::Sender<Reply>,
}

/// The events of one method on a bounded channel of their own ([`CdpRouter::tap`]), so a
/// subscriber that must see every one of them is not crowded out of the shared broadcast by the
/// others; what did not fit is counted.
pub struct Tap {
    events: mpsc::Receiver<CdpEvent>,
    dropped: Arc<AtomicU64>,
}

impl Tap {
    /// The next event; `None` once the router is gone.
    pub async fn recv(&mut self) -> Option<CdpEvent> {
        self.events.recv().await
    }

    /// How many events did not fit since the last call.
    pub fn take_dropped(&self) -> u64 {
        self.dropped.swap(0, Ordering::AcqRel)
    }
}

struct TapEnd {
    method: String,
    events: mpsc::Sender<CdpEvent>,
    dropped: Arc<AtomicU64>,
}

/// The client side of the connection: numbers commands, matches replies, broadcasts events.
pub struct CdpRouter {
    transport: Arc<dyn CdpTransport>,
    next_id: AtomicU64,
    pending: Mutex<HashMap<u64, Pending>>,
    events: broadcast::Sender<CdpEvent>,
    taps: Mutex<Vec<TapEnd>>,
    closed: AtomicBool,
}

impl CdpRouter {
    /// A router writing to `transport`.
    pub fn new(transport: Arc<dyn CdpTransport>) -> Arc<Self> {
        let (events, _) = broadcast::channel(EVENT_BUFFER);
        Arc::new(Self {
            transport,
            next_id: AtomicU64::new(1),
            pending: Mutex::new(HashMap::new()),
            events,
            taps: Mutex::new(Vec::new()),
            closed: AtomicBool::new(false),
        })
    }

    /// Sends `method` (to `session`, or the browser target) and waits for its `result`.
    pub async fn call(
        &self,
        method: &str,
        params: Value,
        session: Option<&str>,
    ) -> Result<Value, EngineError> {
        if self.closed.load(Ordering::Acquire) {
            return Err(EngineError::Closed);
        }
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let mut message = json!({ "id": id, "method": method, "params": params });
        if let Some(session) = session {
            message["sessionId"] = json!(session);
        }
        let frame = encode(&message).map_err(|error| EngineError::Invalid(error.to_string()))?;
        let (reply, answer) = oneshot::channel();
        lock(&self.pending).insert(
            id,
            Pending {
                method: method.to_owned(),
                reply,
            },
        );
        if let Err(error) = self.transport.send(frame) {
            lock(&self.pending).remove(&id);
            return Err(error);
        }
        match tokio::time::timeout(CALL_TIMEOUT, answer).await {
            Ok(Ok(reply)) => reply,
            Ok(Err(_)) => Err(EngineError::Closed),
            Err(_) => {
                lock(&self.pending).remove(&id);
                Err(EngineError::Cdp {
                    method: method.to_owned(),
                    code: 0,
                    message: format!("no answer within {} s", CALL_TIMEOUT.as_secs()),
                })
            }
        }
    }

    /// Handles one message read from the connection: a reply completes its call, an event goes
    /// to every subscriber. Messages that are neither are dropped.
    pub fn on_message(&self, message: Value) {
        if let Some(id) = message.get("id").and_then(Value::as_u64) {
            let Some(pending) = lock(&self.pending).remove(&id) else {
                return;
            };
            let reply = match message.get("error") {
                Some(error) => Err(EngineError::Cdp {
                    method: pending.method,
                    code: error.get("code").and_then(Value::as_i64).unwrap_or(0),
                    message: error
                        .get("message")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_owned(),
                }),
                None => Ok(message.get("result").cloned().unwrap_or(Value::Null)),
            };
            // The caller may have timed out and gone; nothing to do then.
            let _ = pending.reply.send(reply);
            return;
        }
        let Some(method) = message.get("method").and_then(Value::as_str) else {
            return;
        };
        let event = CdpEvent {
            method: method.to_owned(),
            params: message.get("params").cloned().unwrap_or(Value::Null),
            session_id: message
                .get("sessionId")
                .and_then(Value::as_str)
                .map(str::to_owned),
        };
        lock(&self.taps).retain(|tap| {
            if tap.method != event.method {
                return true;
            }
            match tap.events.try_send(event.clone()) {
                Ok(()) => true,
                Err(mpsc::error::TrySendError::Full(_)) => {
                    tap.dropped.fetch_add(1, Ordering::AcqRel);
                    true
                }
                Err(mpsc::error::TrySendError::Closed(_)) => false,
            }
        });
        // No subscriber is not an error.
        let _ = self.events.send(event);
    }

    /// The connection is gone: every waiting call fails with [`EngineError::Closed`], later
    /// calls too.
    pub fn on_closed(&self) {
        self.closed.store(true, Ordering::Release);
        for (_, pending) in lock(&self.pending).drain() {
            let _ = pending.reply.send(Err(EngineError::Closed));
        }
    }

    /// Whether the connection is gone.
    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Acquire)
    }

    /// Every event from now on.
    pub fn subscribe(&self) -> broadcast::Receiver<CdpEvent> {
        self.events.subscribe()
    }

    /// The events of `method` from now on, up to `capacity` waiting (they reach the broadcast
    /// too); a tap dropped is let go at its method's next event.
    pub fn tap(&self, method: &str, capacity: usize) -> Tap {
        let (sender, events) = mpsc::channel(capacity.max(1));
        let dropped = Arc::new(AtomicU64::new(0));
        lock(&self.taps).push(TapEnd {
            method: method.to_owned(),
            events: sender,
            dropped: Arc::clone(&dropped),
        });
        Tap { events, dropped }
    }
}

/// The crate's mutex lock, under its old path: `rb_util::lock` takes over the data of a thread
/// that panicked while holding it, so the maps here stay consistent at every await point.
pub(crate) use rb_util::lock;

#[cfg(test)]
#[path = "router_tests.rs"]
mod tests;
