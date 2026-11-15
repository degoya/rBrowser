//! The client of the stdio protocol: requests with a timeout, `$/progress` to the request it
//! belongs to, other notifications to subscribers, `$/cancel` when a request times out or its
//! future is dropped. When the sidecar's stdout ends, every running request ends with
//! [`SidecarError::Exited`] instead of hanging.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::sync::{broadcast, mpsc, oneshot, watch};

use crate::LOG_TARGET;
use crate::error::SidecarError;
use crate::framing::{LineRead, read_line};
use crate::rpc::{
    CANCEL, Incoming, MAX_LINE_BYTES, Notification, PROGRESS, notification_line, parse_line,
    request_line,
};

/// Notifications buffered per subscriber before the oldest are dropped.
const NOTIFICATION_CAPACITY: usize = 256;

type Reply = Result<Value, SidecarError>;

struct Pending {
    method: String,
    reply: oneshot::Sender<Reply>,
    progress: Option<mpsc::UnboundedSender<Value>>,
}

/// What the reader task and the client share. The line sender is not in here: when the last
/// [`RpcClient`] goes, the writer task ends and closes the sidecar's stdin, so it exits.
/// [`RpcClient::close_input`] closes it at once, whoever still holds a client.
struct Shared {
    next_id: AtomicU64,
    pending: Mutex<HashMap<u64, Pending>>,
    closed: watch::Sender<bool>,
    /// Set by [`RpcClient::close_input`]: the writer closes the sidecar's stdin.
    stop_writing: watch::Sender<bool>,
    /// When the connection closed (tokio's clock), for the supervisor's crash count.
    closed_at: Mutex<Option<Instant>>,
    notifications: broadcast::Sender<Notification>,
}

impl Shared {
    fn pending(&self) -> MutexGuard<'_, HashMap<u64, Pending>> {
        self.pending.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Marks the connection closed and ends every running request.
    fn close(&self) {
        self.closed_at
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get_or_insert_with(|| tokio::time::Instant::now().into_std());
        self.closed.send_replace(true);
        let drained: Vec<Pending> = self.pending().drain().map(|(_, p)| p).collect();
        for pending in drained {
            let _ = pending.reply.send(Err(SidecarError::Exited));
        }
    }
}

/// A connection to one sidecar process. Cheap to clone.
#[derive(Clone)]
pub struct RpcClient {
    shared: Arc<Shared>,
    out: mpsc::UnboundedSender<String>,
}

impl RpcClient {
    /// Starts the reader and writer tasks on the current tokio runtime.
    pub fn start<R, W>(reader: R, writer: W) -> Self
    where
        R: AsyncRead + Send + Unpin + 'static,
        W: AsyncWrite + Send + Unpin + 'static,
    {
        let (out, lines) = mpsc::unbounded_channel();
        let (closed, _) = watch::channel(false);
        let (stop_writing, stop) = watch::channel(false);
        let (notifications, _) = broadcast::channel(NOTIFICATION_CAPACITY);
        let shared = Arc::new(Shared {
            next_id: AtomicU64::new(1),
            pending: Mutex::new(HashMap::new()),
            closed,
            stop_writing,
            closed_at: Mutex::new(None),
            notifications,
        });
        tokio::spawn(write_loop(writer, lines, stop, Arc::clone(&shared)));
        tokio::spawn(read_loop(BufReader::new(reader), Arc::clone(&shared)));
        Self { shared, out }
    }

    /// Closes the sidecar's stdin now, which asks it to end (`sidecar/src/main.ts`); nothing
    /// more is sent. The answers still come.
    pub fn close_input(&self) {
        self.shared.stop_writing.send_replace(true);
    }

    /// Whether the sidecar's output has ended (it exited or broke the pipe).
    pub fn is_closed(&self) -> bool {
        *self.shared.closed.borrow()
    }

    /// When the sidecar's output ended (it died), on tokio's clock; `None` while it is open.
    pub fn closed_at(&self) -> Option<Instant> {
        *self
            .shared
            .closed_at
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    /// Resolves once the connection is closed.
    pub async fn closed(&self) {
        let mut closed = self.shared.closed.subscribe();
        let _ = closed.wait_for(|closed| *closed).await;
    }

    /// The notifications that belong to no request (events of later modules).
    pub fn subscribe(&self) -> broadcast::Receiver<Notification> {
        self.shared.notifications.subscribe()
    }

    /// A flag that turns `true` once the connection is closed (the process ended).
    pub fn closed_watch(&self) -> watch::Receiver<bool> {
        self.shared.closed.subscribe()
    }

    /// Sends a notification.
    pub fn notify(&self, method: &str, params: &Value) -> Result<(), SidecarError> {
        let line = notification_line(method, params)?;
        self.out.send(line).map_err(|_| SidecarError::Exited)
    }

    /// Sends a request and waits for its answer, at most `timeout` (`None`: no limit). On a
    /// timeout, or when this future is dropped, the sidecar gets `$/cancel`.
    pub async fn request(
        &self,
        method: &str,
        params: Value,
        timeout: Option<Duration>,
    ) -> Result<Value, SidecarError> {
        self.call(method, params, timeout, None).await
    }

    /// [`RpcClient::request`], with the request's `$/progress` parameters sent to `progress`.
    pub async fn request_with_progress(
        &self,
        method: &str,
        params: Value,
        timeout: Option<Duration>,
        progress: mpsc::UnboundedSender<Value>,
    ) -> Result<Value, SidecarError> {
        self.call(method, params, timeout, Some(progress)).await
    }

    async fn call(
        &self,
        method: &str,
        params: Value,
        timeout: Option<Duration>,
        progress: Option<mpsc::UnboundedSender<Value>>,
    ) -> Result<Value, SidecarError> {
        let id = self.shared.next_id.fetch_add(1, Ordering::Relaxed);
        let line = request_line(id, method, &params)?;
        let (reply, answer) = oneshot::channel();
        {
            let mut pending = self.shared.pending();
            if self.is_closed() {
                return Err(SidecarError::Exited);
            }
            let method = method.to_owned();
            pending.insert(
                id,
                Pending {
                    method,
                    reply,
                    progress,
                },
            );
        }
        let mut guard = CancelOnDrop {
            client: self,
            id,
            armed: true,
        };
        if self.out.send(line).is_err() {
            guard.armed = false;
            self.shared.pending().remove(&id);
            return Err(SidecarError::Exited);
        }
        let answer = match timeout {
            Some(limit) => match tokio::time::timeout(limit, answer).await {
                Ok(answer) => answer,
                Err(_) => {
                    let ms = u64::try_from(limit.as_millis()).unwrap_or(u64::MAX);
                    let method = method.to_owned();
                    return Err(SidecarError::Timeout { method, ms });
                }
            },
            None => answer.await,
        };
        guard.armed = false;
        answer.unwrap_or(Err(SidecarError::Exited))
    }
}

/// Cancels a request whose future ends without its answer (timeout or drop).
struct CancelOnDrop<'a> {
    client: &'a RpcClient,
    id: u64,
    armed: bool,
}

impl Drop for CancelOnDrop<'_> {
    fn drop(&mut self) {
        if !self.armed || self.client.shared.pending().remove(&self.id).is_none() {
            return;
        }
        if let Ok(line) = notification_line(CANCEL, &json!({ "id": self.id })) {
            let _ = self.client.out.send(line);
        }
    }
}

async fn write_loop<W: AsyncWrite + Unpin>(
    mut writer: W,
    mut lines: mpsc::UnboundedReceiver<String>,
    mut stop: watch::Receiver<bool>,
    shared: Arc<Shared>,
) {
    loop {
        let line = tokio::select! {
            biased;
            _ = stop.wait_for(|stop| *stop) => break,
            line = lines.recv() => match line {
                Some(line) => line,
                None => break,
            },
        };
        let written = async {
            writer.write_all(line.as_bytes()).await?;
            writer.flush().await
        };
        if let Err(error) = written.await {
            tracing::warn!(target: LOG_TARGET, %error, "cannot write to the sidecar");
            shared.close();
            return;
        }
    }
    let _ = writer.shutdown().await;
}

async fn read_loop<R: AsyncRead + Unpin>(mut reader: BufReader<R>, shared: Arc<Shared>) {
    let mut buf = Vec::new();
    loop {
        match read_line(&mut reader, &mut buf, MAX_LINE_BYTES).await {
            Ok(LineRead::Line) => dispatch(&shared, &buf),
            Ok(LineRead::TooLong) => {
                tracing::warn!(target: LOG_TARGET, "skipped a sidecar message over the size limit");
            }
            Ok(LineRead::Eof) => break,
            Err(error) => {
                tracing::warn!(target: LOG_TARGET, %error, "cannot read from the sidecar");
                break;
            }
        }
    }
    shared.close();
}

fn dispatch(shared: &Shared, line: &[u8]) {
    let text = String::from_utf8_lossy(line);
    match parse_line(&text) {
        Ok(Incoming::Response { id, outcome }) => {
            let Some(pending) = shared.pending().remove(&id) else {
                tracing::debug!(target: LOG_TARGET, id, "an answer to a cancelled request");
                return;
            };
            let reply = outcome.map_err(|error| SidecarError::Remote {
                method: pending.method,
                code: error.code,
                message: error.message,
                data: error.data.map(Box::new),
            });
            let _ = pending.reply.send(reply);
        }
        Ok(Incoming::Notification(notification)) => {
            if notification.method == PROGRESS
                && let Some(id) = notification.params.get("id").and_then(Value::as_u64)
                && let Some(progress) = shared.pending().get(&id).and_then(|p| p.progress.clone())
            {
                let _ = progress.send(notification.params);
                return;
            }
            let _ = shared.notifications.send(notification);
        }
        Err(error) => {
            let error = rb_diag::redact(&error.to_string());
            tracing::warn!(target: LOG_TARGET, %error, "skipped a sidecar line");
        }
    }
}

#[cfg(test)]
#[path = "client_tests.rs"]
mod tests;
