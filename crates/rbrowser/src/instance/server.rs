//! The instance's side of the channel: a thread of its own with a small runtime listens, writes
//! the instance file once it does, and answers up to [`MAX_CONCURRENT`] connections at once (a
//! screenshot can take minutes; an open must not wait for it) - per connection one line in, checked
//! by [`protocol::check`], handed to the [`Handler`], one line out (at most
//! [`protocol::MAX_REPLY`] bytes). [`Listening::stop`] ends it before a restart (RB-094-23).

use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{
    AsyncBufReadExt as _, AsyncRead, AsyncReadExt as _, AsyncWrite, AsyncWriteExt as _,
};

use serde_json::Value;

use super::protocol::{self, MAX_LINE, MAX_REPLY, Refusal, Reply, Request};
use super::{channel, file};

/// What the instance does with a checked request: `Ok` with what it answers (`screenshot`),
/// `Err` with why it failed.
pub type Handler = Arc<dyn Fn(Request) -> Result<Option<Value>, String> + Send + Sync>;

/// How long a client may take to send its line.
const READ_TIMEOUT: Duration = Duration::from_secs(5);
/// The most connections answered at once; the next waits for a free slot.
const MAX_CONCURRENT: usize = 4;
/// How long [`Listening::stop`] waits for the channel to close.
const STOP_WAIT: Duration = Duration::from_secs(2);

/// The running listener. Dropped, it keeps listening as long as the process runs.
pub struct Listening {
    local: PathBuf,
    path: PathBuf,
    nonce: String,
    stop: Arc<tokio::sync::Notify>,
    closed: std::sync::mpsc::Receiver<()>,
}

impl Listening {
    /// Stops listening, so no further call reaches this process: removes the instance file
    /// while it is still this one's, ends the listener and its open connections, and waits up
    /// to [`STOP_WAIT`] for the channel to close. The quit of `app.restart` runs it before the
    /// next start, which would otherwise hand its start over to this ending process.
    pub fn stop(self) {
        if file::read(&self.path, &self.local).is_some_and(|written| written.nonce == self.nonce)
            && let Err(error) = std::fs::remove_file(&self.path)
        {
            tracing::warn!(%error, "the instance file was not removed");
        }
        self.stop.notify_one();
        if self.closed.recv_timeout(STOP_WAIT).is_err() {
            tracing::warn!("the instance channel did not close in time");
        }
    }
}

/// Starts listening for the project at `root` and writes its instance file under `local`;
/// returns once both are done (or failed). The listener runs as long as the process, or until
/// [`Listening::stop`].
pub fn start(local: &Path, root: &Path, handler: Handler) -> io::Result<Listening> {
    let dir = file::instances_dir(local);
    let path = file::file_path(local, root);
    let nonce = protocol::new_nonce()?;
    let instance = file::InstanceFile {
        version: file::FILE_VERSION,
        pid: std::process::id(),
        endpoint: channel::endpoint_for(&dir, &file::key(root)),
        nonce: nonce.clone(),
        project: root.to_string_lossy().into_owned(),
    };
    let stop = Arc::new(tokio::sync::Notify::new());
    let (written, kept, serving) = (path.clone(), nonce.clone(), stop.clone());
    let (closing, closed) = std::sync::mpsc::channel();
    let (ready, started) = std::sync::mpsc::channel();
    std::thread::Builder::new()
        .name("rb-instance".to_owned())
        .spawn(move || {
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(error) => {
                    let _ = ready.send(Err(error));
                    return;
                }
            };
            let served = runtime.block_on(async move {
                let listener = match channel::bind(&instance.endpoint)
                    .and_then(|listener| file::write(&written, &instance).map(|()| listener))
                {
                    Ok(listener) => listener,
                    Err(error) => {
                        let _ = ready.send(Err(error));
                        return false;
                    }
                };
                let _ = ready.send(Ok(()));
                serve(listener, nonce, handler, &serving).await;
                true
            });
            if served {
                // Stopped: the connections still open end with their tasks; a handler still
                // running finishes on its own thread.
                runtime.shutdown_timeout(Duration::ZERO);
                let _ = closing.send(());
            }
        })?;
    started
        .recv()
        .unwrap_or_else(|_| Err(io::Error::other("the instance thread ended")))?;
    Ok(Listening {
        local: local.to_path_buf(),
        path,
        nonce: kept,
        stop,
        closed,
    })
}

/// Answers connections, each in a task of its own, until `stop` is notified.
async fn serve(
    mut listener: channel::Listener,
    nonce: String,
    handler: Handler,
    stop: &tokio::sync::Notify,
) {
    let nonce: Arc<str> = nonce.into();
    let slots = Arc::new(tokio::sync::Semaphore::new(MAX_CONCURRENT));
    loop {
        let slot = tokio::select! {
            () = stop.notified() => return,
            slot = slots.clone().acquire_owned() => slot,
        };
        let Ok(slot) = slot else {
            return;
        };
        let accepted = tokio::select! {
            () = stop.notified() => return,
            accepted = listener.accept() => accepted,
        };
        match accepted {
            Ok(conn) => {
                let (nonce, handler) = (nonce.clone(), handler.clone());
                tokio::spawn(async move {
                    if let Err(refusal) = serve_one(conn, &nonce, &handler).await {
                        tracing::warn!(?refusal, "a request to this instance was refused");
                    }
                    drop(slot);
                });
            }
            Err(error) => {
                tracing::warn!(%error, "the instance channel failed to accept");
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
        }
    }
}

/// Reads one request from `stream`, checks it against `nonce`, runs it and answers.
pub async fn serve_one<S: AsyncRead + AsyncWrite + Unpin>(
    stream: S,
    nonce: &str,
    handler: &Handler,
) -> Result<Request, Refusal> {
    let mut reader = tokio::io::BufReader::new(stream);
    let result = match read_line(&mut reader).await {
        Some(line) => protocol::check(&line, nonce),
        None => Err(Refusal::BadRequest),
    };
    let (mut result, reply) = match result {
        Ok(request) => {
            let handler = handler.clone();
            let run = request.clone();
            match tokio::task::spawn_blocking(move || handler(run)).await {
                Ok(Ok(None)) => (Ok(request), Reply::ok()),
                Ok(Ok(Some(answer))) => (Ok(request), Reply::answered(answer)),
                Ok(Err(error)) => {
                    tracing::warn!(%error, "a handed-over request failed");
                    (Err(Refusal::Failed), Reply::failed(&error))
                }
                Err(_) => (Err(Refusal::Failed), Reply::refused(Refusal::Failed)),
            }
        }
        Err(refusal) => (Err(refusal), Reply::refused(refusal)),
    };
    let mut line = protocol::encode_reply(&reply);
    if line.len() > MAX_REPLY {
        tracing::warn!(bytes = line.len(), "an answer was too long for the channel");
        line = protocol::encode_reply(&Reply::failed("the answer is too long"));
        result = Err(Refusal::Failed);
    }
    let stream = reader.get_mut();
    let _ = stream.write_all(&line).await;
    let _ = stream.flush().await;
    let _ = stream.shutdown().await;
    result
}

/// One line of at most `MAX_LINE` bytes, read within [`READ_TIMEOUT`]; `None` when it is
/// longer, unfinished or late.
pub async fn read_line<R: AsyncRead + Unpin>(
    reader: &mut tokio::io::BufReader<R>,
) -> Option<Vec<u8>> {
    read_line_within(reader, MAX_LINE, READ_TIMEOUT).await
}

/// [`read_line`] with its limits given.
pub async fn read_line_within<R: AsyncRead + Unpin>(
    reader: &mut tokio::io::BufReader<R>,
    max: usize,
    timeout: Duration,
) -> Option<Vec<u8>> {
    let mut line = Vec::new();
    let limit = u64::try_from(max).unwrap_or(u64::MAX).saturating_add(1);
    let read = tokio::time::timeout(
        timeout,
        (&mut *reader).take(limit).read_until(b'\n', &mut line),
    )
    .await
    .ok()?
    .ok()?;
    (read > 0 && read <= max && line.last() == Some(&b'\n')).then_some(line)
}

#[cfg(test)]
#[path = "server_tests.rs"]
mod tests;
