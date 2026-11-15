//! The proxy's listener on 127.0.0.1: one task per connection reads the request head, lets
//! [`rb_cdp::proxy::decide`] judge it (loopback host, origin, token, endpoint), answers the JSON
//! endpoints itself from the pipe and hands a WebSocket upgrade to [`super::connection`]. The
//! token lives here and can be rotated: every connection ends then, and only the new token gets
//! in. Nothing logged here carries the request's path or headers - the token may be in either.

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use rb_cdp::proxy::{
    Base, Decision, Endpoint, Refusal, decide, list_body, new_browser_id, protocol_body,
    version_body,
};
use rb_cdp::request::{HeadError, MAX_HEAD, head_end, parse_head};
use rb_cdp::target_filter::visible_infos;
use rb_cdp::{ProxyToken, TargetFilter};
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::runtime::Handle;
use tokio::sync::watch;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::handshake::derive_accept_key;
use tokio_tungstenite::tungstenite::protocol::Role;

use crate::cdp::{CdpRouter, lock};

/// How long a client may take to send its request head.
const HEAD_TIMEOUT: Duration = Duration::from_secs(10);

/// A target's scale (CSS px to widget DIP), by target id.
pub type ScaleOf = Arc<dyn Fn(&str) -> Option<f64> + Send + Sync>;
/// What the proxy tells when a client leaves: the targets it attached.
pub type OnEnd = Arc<dyn Fn(Vec<String>) + Send + Sync>;

/// What the app gives the proxy.
#[derive(Clone)]
pub struct Hooks {
    /// The scale of a view's target, for `Input.*`.
    pub scale: ScaleOf,
    /// A client left: re-apply the emulation of the views it touched.
    pub on_end: OnEnd,
    /// The hidden targets.
    pub filter: &'static dyn TargetFilter,
}

/// The running proxy; dropping it stops the listener and ends every connection.
pub struct ProxyServer {
    port: u16,
    browser_id: String,
    pub(super) router: Arc<CdpRouter>,
    token: Mutex<ProxyToken>,
    /// Counts token rotations and the stop; every change ends the connections that are open.
    revoked: watch::Sender<u64>,
    /// [`Self::stop`] was called: the listener closes.
    stopped: AtomicBool,
    pub(super) hooks: Hooks,
    /// What lives as long as the proxy and every connection of it ([`Self::keep_until_dropped`]).
    kept: Mutex<Vec<Box<dyn Send + Sync>>>,
}

impl ProxyServer {
    /// Listens on `127.0.0.1:<port>` (`None` or 0: a port the OS picks) and serves on
    /// `runtime`.
    ///
    /// # Errors
    ///
    /// The port is taken, or the OS gives no randomness for the browser id.
    pub fn start(
        runtime: &Handle,
        router: Arc<CdpRouter>,
        port: Option<u16>,
        token: ProxyToken,
        hooks: Hooks,
    ) -> std::io::Result<Arc<Self>> {
        let address = SocketAddr::from((Ipv4Addr::LOCALHOST, port.unwrap_or(0)));
        let listener = std::net::TcpListener::bind(address)?;
        listener.set_nonblocking(true)?;
        let port = listener.local_addr()?.port();
        let listener = {
            let _entered = runtime.enter();
            TcpListener::from_std(listener)?
        };
        let (revoked, _) = watch::channel(0);
        let server = Arc::new(Self {
            port,
            browser_id: new_browser_id()?,
            router,
            token: Mutex::new(token),
            revoked,
            stopped: AtomicBool::new(false),
            hooks,
            kept: Mutex::new(Vec::new()),
        });
        let stop = server.revoked.subscribe();
        runtime.spawn(accept_loop(listener, Arc::downgrade(&server), stop));
        Ok(server)
    }

    /// The port it listens on.
    pub fn port(&self) -> u16 {
        self.port
    }

    /// The id of `/devtools/browser/<id>`.
    pub fn browser_id(&self) -> &str {
        &self.browser_id
    }

    /// The token now: for an authenticated MCP client and the shell's copy command only.
    pub fn token(&self) -> ProxyToken {
        lock(&self.token).clone()
    }

    /// Replaces the token and ends every open connection.
    pub fn rotate(&self, token: ProxyToken) {
        *lock(&self.token) = token;
        self.revoked
            .send_modify(|count| *count = count.wrapping_add(1));
    }

    /// Stops listening and ends every open connection.
    pub fn stop(&self) {
        self.stopped.store(true, Ordering::Release);
        self.revoked
            .send_modify(|count| *count = count.wrapping_add(1));
    }

    /// Keeps `value` until the proxy is dropped - which is after its last connection ended,
    /// since every connection holds the proxy. A flow's view slot (`flows::scope`) is freed this
    /// way, never while a connection of a stopped proxy could still judge a target with it.
    pub fn keep_until_dropped(&self, value: Box<dyn Send + Sync>) {
        lock(&self.kept).push(value);
    }

    /// A receiver that changes when the token is rotated or the proxy stops.
    pub(super) fn revocations(&self) -> watch::Receiver<u64> {
        self.revoked.subscribe()
    }
}

async fn accept_loop(
    listener: TcpListener,
    server: Weak<ProxyServer>,
    mut stop: watch::Receiver<u64>,
) {
    loop {
        tokio::select! {
            accepted = listener.accept() => {
                let stream = match accepted {
                    Ok((stream, _)) => stream,
                    Err(error) => {
                        tracing::warn!(%error, "a CDP proxy connection was not accepted");
                        continue;
                    }
                };
                let Some(server) = server.upgrade() else {
                    return;
                };
                tokio::spawn(serve(stream, server));
            }
            // A rotation goes on listening; a stop or the proxy dropped ends it.
            changed = stop.changed() => {
                let stopped = server
                    .upgrade()
                    .is_none_or(|server| server.stopped.load(Ordering::Acquire));
                if changed.is_err() || stopped {
                    return;
                }
            }
        }
    }
}

async fn serve(mut stream: TcpStream, server: Arc<ProxyServer>) {
    let head = match tokio::time::timeout(HEAD_TIMEOUT, read_head(&mut stream)).await {
        Ok(Ok(head)) => head,
        Ok(Err(refusal)) => {
            let _ = refuse(&mut stream, refusal).await;
            return;
        }
        Err(_) => return,
    };
    let head = match parse_head(&head) {
        Ok(head) => head,
        Err(HeadError::TooLarge) => {
            let _ = refuse(&mut stream, Refusal::TooLarge).await;
            return;
        }
        Err(HeadError::Malformed) => {
            let _ = refuse(&mut stream, Refusal::BadRequest).await;
            return;
        }
    };
    let decision = decide(&head, &lock(&server.token), &server.browser_id);
    match decision {
        Decision::Refuse(refusal) => {
            // Only the status: the path and the headers may carry a token.
            tracing::debug!(status = refusal.status(), "a CDP proxy request was refused");
            let _ = refuse(&mut stream, refusal).await;
        }
        Decision::Http(endpoint, base) => {
            let body = json_answer(&server, &endpoint, &base).await;
            let _ = respond(
                &mut stream,
                200,
                "OK",
                "application/json",
                &body.to_string(),
            )
            .await;
        }
        Decision::Upgrade(endpoint, key) => {
            let accept = derive_accept_key(key.as_bytes());
            let switching = format!(
                "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {accept}\r\n\r\n"
            );
            if stream.write_all(switching.as_bytes()).await.is_err() {
                return;
            }
            let ws = WebSocketStream::from_raw_socket(stream, Role::Server, None).await;
            super::connection::run(ws, endpoint, server).await;
        }
    }
}

/// The request head, up to its blank line.
async fn read_head(stream: &mut TcpStream) -> Result<Vec<u8>, Refusal> {
    let mut buffer = Vec::with_capacity(1024);
    let mut chunk = [0u8; 1024];
    loop {
        let read = stream
            .read(&mut chunk)
            .await
            .map_err(|_| Refusal::BadRequest)?;
        if read == 0 {
            return Err(Refusal::BadRequest);
        }
        buffer.extend_from_slice(&chunk[..read]);
        if let Some(end) = head_end(&buffer) {
            buffer.truncate(end);
            return Ok(buffer);
        }
        if buffer.len() > MAX_HEAD {
            return Err(Refusal::TooLarge);
        }
    }
}

/// Builds a `/json/*` answer from the browser's result.
type ReadAnswer = fn(&ProxyServer, &Base, &Value) -> Value;

/// The JSON of a `/json/*` endpoint, read from the pipe; the browser's error as `{ "error" }`.
async fn json_answer(server: &ProxyServer, endpoint: &Endpoint, base: &Base) -> Value {
    let (method, read): (&str, ReadAnswer) = match endpoint {
        Endpoint::Version => ("Browser.getVersion", |server, base, version| {
            version_body(base, &server.browser_id, version)
        }),
        Endpoint::List => ("Target.getTargets", |server, base, targets| {
            let infos = targets
                .get("targetInfos")
                .and_then(Value::as_array)
                .map(|infos| visible_infos(server.hooks.filter, infos))
                .unwrap_or_default();
            list_body(base, &infos)
        }),
        Endpoint::Protocol => ("Schema.getDomains", |_, _, domains| protocol_body(domains)),
        Endpoint::Browser | Endpoint::Page(_) => return json!({}),
    };
    match server.router.call(method, json!({}), None).await {
        Ok(result) => read(server, base, &result),
        Err(error) => json!({ "error": error.to_string() }),
    }
}

async fn refuse(stream: &mut TcpStream, refusal: Refusal) -> std::io::Result<()> {
    let reason = refusal.reason();
    let phrase = reason.split(':').next().unwrap_or(reason);
    respond(stream, refusal.status(), phrase, "text/plain", reason).await
}

async fn respond(
    stream: &mut TcpStream,
    status: u16,
    phrase: &str,
    content_type: &str,
    body: &str,
) -> std::io::Result<()> {
    let answer = format!(
        "HTTP/1.1 {status} {phrase}\r\nContent-Type: {content_type}; charset=UTF-8\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(answer.as_bytes()).await?;
    stream.shutdown().await
}

#[cfg(test)]
#[path = "server_tests.rs"]
mod tests;
