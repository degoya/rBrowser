//! The internal DevTools endpoint: a WebSocket server on 127.0.0.1 (a random port) that the
//! docked frontend connects to (`devtools://devtools/bundled/inspector.html?ws=...`). It has
//! no `/json` list and no other path: each [`Ticket`] admits exactly one connection, at
//! `/devtools/<token>`, to the one page target it was granted for. The connection attaches its
//! own flat session over the pipe and relays through [`Relay`]; dropping the ticket ends it.
//! When a connection ends its session is detached and `on_end` hears of the view, so the
//! host re-applies the view's emulation (DevTools' device toolbar may have changed it).
//! An "Inspect element" request ([`super::inspect`]) waits in the endpoint's [`InspectQueue`]
//! until the view's frontend has enabled the overlay, then reaches it as
//! `Overlay.inspectNodeRequested`.

use std::fmt;
use std::future::Future;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex, Weak};
use std::task::Poll;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use rb_engine::{EngineError, ViewId};
use serde_json::{Value, json};
use tokio::net::{TcpListener, TcpStream};
use tokio::runtime::Handle;
use tokio::sync::broadcast::error::TryRecvError;
use tokio::sync::{mpsc, oneshot};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::handshake::server::{ErrorResponse, Request, Response};
use tokio_tungstenite::tungstenite::http::StatusCode;

use super::inspect::InspectQueue;
use super::relay::{Call, FromFrontend, Relay, ToFrontend};
use super::token::Token;
use crate::cdp::{CdpEvent, CdpRouter, lock};

/// The path prefix of a connection; the token follows.
pub const PATH_PREFIX: &str = "/devtools/";
/// The only origin a browser may connect from: DevTools' own pages.
pub const FRONTEND_ORIGIN: &str = "devtools://devtools";
/// How long a client may take for the WebSocket handshake.
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);

/// What the endpoint tells when a connection ends: the view it inspected.
pub type OnEnd = Arc<dyn Fn(ViewId) + Send + Sync>;

/// The docked frontend's URL for a connection to `port` with `token`.
pub fn frontend_url(port: u16, token: &Token) -> String {
    format!(
        "devtools://devtools/bundled/inspector.html?ws=127.0.0.1:{port}{PATH_PREFIX}{}",
        token.expose()
    )
}

struct Grant {
    token: Token,
    view: ViewId,
    target_id: String,
    stop: oneshot::Receiver<()>,
}

/// One admitted connection; dropping it withdraws the grant or ends the connection.
pub struct Ticket {
    url: String,
    _stop: oneshot::Sender<()>,
}

impl Ticket {
    /// The frontend URL with the token: for the dock's view only, never logged.
    pub fn frontend_url(&self) -> &str {
        &self.url
    }
}

impl fmt::Debug for Ticket {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Ticket(<redacted>)")
    }
}

/// The endpoint.
pub struct Endpoint {
    port: u16,
    router: Arc<CdpRouter>,
    grants: Mutex<Vec<Grant>>,
    on_end: OnEnd,
    inspect: Arc<InspectQueue>,
}

impl Endpoint {
    /// Listens on 127.0.0.1 (a port the OS picks) and serves on `runtime`.
    pub fn start(
        runtime: &Handle,
        router: Arc<CdpRouter>,
        on_end: OnEnd,
    ) -> std::io::Result<Arc<Self>> {
        let listener = std::net::TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))?;
        listener.set_nonblocking(true)?;
        let port = listener.local_addr()?.port();
        let listener = {
            let _entered = runtime.enter();
            TcpListener::from_std(listener)?
        };
        let endpoint = Arc::new(Self {
            port,
            router,
            grants: Mutex::new(Vec::new()),
            on_end,
            inspect: Arc::new(InspectQueue::default()),
        });
        runtime.spawn(accept_loop(listener, Arc::downgrade(&endpoint)));
        Ok(endpoint)
    }

    /// The port it listens on.
    pub fn port(&self) -> u16 {
        self.port
    }

    /// The nodes waiting to be revealed in a view's frontend (RB-080-43).
    pub fn inspect(&self) -> &Arc<InspectQueue> {
        &self.inspect
    }

    /// Admits one connection to the page target `target_id` of `view`; `None` without
    /// randomness for the token.
    pub fn grant(&self, view: ViewId, target_id: &str) -> Option<Ticket> {
        let token = Token::generate()?;
        let url = frontend_url(self.port, &token);
        let (stop_sender, stop) = oneshot::channel();
        let mut grants = lock(&self.grants);
        grants.retain_mut(ticket_held);
        grants.push(Grant {
            token,
            view,
            target_id: target_id.to_owned(),
            stop,
        });
        Some(Ticket {
            url,
            _stop: stop_sender,
        })
    }

    /// Takes the grant `path` names, when its ticket is still held and the origin is
    /// DevTools' own (or none, for a client that is no browser).
    fn take_grant(&self, path: &str, origin: Option<&str>) -> Option<Grant> {
        if origin.is_some_and(|origin| origin != FRONTEND_ORIGIN) {
            return None;
        }
        let offered = path.strip_prefix(PATH_PREFIX)?;
        let mut grants = lock(&self.grants);
        grants.retain_mut(ticket_held);
        // Every grant is compared, so the time tells nothing about which one is near.
        let found = grants
            .iter()
            .enumerate()
            .fold(None, |found, (index, grant)| {
                if grant.token.matches(offered) {
                    Some(index)
                } else {
                    found
                }
            });
        found.map(|index| grants.swap_remove(index))
    }
}

/// Whether the grant's ticket is still held (a dropped ticket closes the channel).
fn ticket_held(grant: &mut Grant) -> bool {
    matches!(
        grant.stop.try_recv(),
        Err(oneshot::error::TryRecvError::Empty)
    )
}

async fn accept_loop(listener: TcpListener, endpoint: Weak<Endpoint>) {
    loop {
        let stream = match listener.accept().await {
            Ok((stream, _)) => stream,
            Err(error) => {
                tracing::warn!(%error, "a DevTools connection was not accepted");
                continue;
            }
        };
        let Some(endpoint) = endpoint.upgrade() else {
            return;
        };
        tokio::spawn(serve(stream, endpoint));
    }
}

#[allow(
    clippy::result_large_err,
    reason = "the handshake callback's error type is tungstenite's `ErrorResponse`"
)]
async fn serve(stream: TcpStream, endpoint: Arc<Endpoint>) {
    let taken: Arc<Mutex<Option<Grant>>> = Arc::new(Mutex::new(None));
    let slot = taken.clone();
    let check = {
        let endpoint = endpoint.clone();
        move |request: &Request, response: Response| -> Result<Response, ErrorResponse> {
            let origin = request
                .headers()
                .get("origin")
                .and_then(|value| value.to_str().ok());
            match endpoint.take_grant(request.uri().path(), origin) {
                Some(grant) => {
                    *lock(&slot) = Some(grant);
                    Ok(response)
                }
                None => {
                    let mut refused = ErrorResponse::new(None);
                    *refused.status_mut() = StatusCode::UNAUTHORIZED;
                    Err(refused)
                }
            }
        }
    };
    let handshake = tokio_tungstenite::accept_hdr_async(stream, check);
    let ws = match tokio::time::timeout(HANDSHAKE_TIMEOUT, handshake).await {
        Ok(Ok(ws)) => ws,
        Ok(Err(error)) => {
            // The path holds the token: only the error kind is logged.
            tracing::warn!(error = %error_kind(&error), "a DevTools connection was refused");
            return;
        }
        Err(_) => {
            tracing::warn!("a DevTools connection did not complete its handshake");
            return;
        }
    };
    let Some(grant) = lock(&taken).take() else {
        return;
    };
    let view = grant.view;
    relay_connection(ws, grant, &endpoint.router, &endpoint.inspect).await;
    (endpoint.on_end)(view);
}

fn error_kind(error: &tokio_tungstenite::tungstenite::Error) -> &'static str {
    use tokio_tungstenite::tungstenite::Error;
    match error {
        Error::Http(_) => "refused",
        Error::Protocol(_) => "protocol",
        Error::Io(_) => "io",
        _ => "other",
    }
}

type WsStream = tokio_tungstenite::WebSocketStream<TcpStream>;
type Reply = (Call, Result<Value, EngineError>);

/// Attaches the page, relays until either side ends, detaches. An inspect request for the view
/// goes to the frontend once it enabled the overlay (`inspect`, RB-080-43).
async fn relay_connection(
    ws: WsStream,
    mut grant: Grant,
    router: &Arc<CdpRouter>,
    inspect: &InspectQueue,
) {
    let mut events = router.subscribe();
    let attached = router
        .call(
            "Target.attachToTarget",
            json!({ "targetId": grant.target_id, "flatten": true }),
            None,
        )
        .await;
    let session = match attached.ok().and_then(|reply| {
        reply
            .get("sessionId")
            .and_then(Value::as_str)
            .map(str::to_owned)
    }) {
        Some(session) => session,
        None => {
            tracing::warn!(view = %grant.view, "DevTools could not attach to the view");
            return;
        }
    };
    let mut relay = Relay::new(session.clone(), grant.target_id.clone());
    let (mut sink, mut stream) = ws.split();
    let (replies, mut answered) = mpsc::unbounded_channel::<Reply>();
    let mut outbox = Vec::new();
    let mut gone = false;
    loop {
        tokio::select! {
            _ = &mut grant.stop => break,
            frame = stream.next() => match frame {
                Some(Ok(Message::Text(text))) => {
                    match relay.read_message(text.as_str()) {
                        FromFrontend::Call(call) => {
                            if let Some((call, reply)) = send_call(router, call, &replies).await {
                                gone |= drain_events(&mut events, &mut relay, &mut outbox);
                                relay.on_answered(&call, reply.is_ok());
                                outbox.push(Relay::answer(&call, reply));
                            }
                        }
                        FromFrontend::Answer(answer) => outbox.push(answer),
                        FromFrontend::Drop => {}
                    }
                }
                Some(Ok(Message::Close(_))) | Some(Err(_)) | None => break,
                Some(Ok(_)) => {}
            },
            Some((call, reply)) = answered.recv() => {
                // Every event the browser sent before this answer goes first.
                gone |= drain_events(&mut events, &mut relay, &mut outbox);
                relay.on_answered(&call, reply.is_ok());
                outbox.push(Relay::answer(&call, reply));
            }
            event = events.recv() => match event {
                Ok(event) => gone |= relay_event(&event, &mut relay, &mut outbox),
                Err(tokio::sync::broadcast::error::RecvError::Lagged(missed)) => {
                    tracing::warn!(missed, "DevTools fell behind; events were dropped");
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            },
            () = inspect.changed() => {}
        }
        // Peeked, not taken: a request the flush does not deliver waits for the next
        // connection; one replaced meanwhile stays for the next round.
        let revealed = if relay.overlay_enabled() {
            inspect.pending(grant.view)
        } else {
            None
        };
        if let Some(node) = revealed {
            outbox.push(Relay::inspect_event(node));
        }
        if flush(&mut sink, &mut outbox).await.is_err() || gone {
            break;
        }
        if let Some(node) = revealed {
            inspect.take_if(grant.view, node);
        }
    }
    let _ = sink.close().await;
    let _ = router
        .call(
            "Target.detachFromTarget",
            json!({ "sessionId": session }),
            None,
        )
        .await;
}

/// Writes `call` to the pipe now, so the frontend's order holds ([`send_ordered`]).
async fn send_call(
    router: &Arc<CdpRouter>,
    call: Call,
    replies: &mpsc::UnboundedSender<Reply>,
) -> Option<Reply> {
    let (method, params, session) = (
        call.method.clone(),
        call.params.clone(),
        call.session.clone(),
    );
    send_ordered(router, method, params, Some(session), call, replies).await
}

/// Writes `method` to the pipe now, so a relay keeps its client's order; the answer comes back
/// at once when the browser already gave it, else later through `replies` with `tag` (the
/// internal DevTools endpoint and the CDP proxy, RB-030-06).
pub(crate) async fn send_ordered<T: Send + 'static>(
    router: &Arc<CdpRouter>,
    method: String,
    params: Value,
    session: Option<String>,
    tag: T,
    replies: &mpsc::UnboundedSender<(T, Result<Value, EngineError>)>,
) -> Option<(T, Result<Value, EngineError>)> {
    let router = router.clone();
    let mut pending =
        Box::pin(async move { router.call(&method, params, session.as_deref()).await });
    // The first poll runs `CdpRouter::call` up to its wait, after the message is written.
    match std::future::poll_fn(|context| Poll::Ready(pending.as_mut().poll(context))).await {
        Poll::Ready(reply) => Some((tag, reply)),
        Poll::Pending => {
            let replies = replies.clone();
            tokio::spawn(async move {
                let reply = pending.await;
                let _ = replies.send((tag, reply));
            });
            None
        }
    }
}

/// Takes the events already received; `true` when the page's session is gone.
fn drain_events(
    events: &mut tokio::sync::broadcast::Receiver<CdpEvent>,
    relay: &mut Relay,
    outbox: &mut Vec<Value>,
) -> bool {
    let mut gone = false;
    loop {
        match events.try_recv() {
            Ok(event) => gone |= relay_event(&event, relay, outbox),
            Err(TryRecvError::Lagged(missed)) => {
                tracing::warn!(missed, "DevTools fell behind; events were dropped");
            }
            Err(TryRecvError::Empty | TryRecvError::Closed) => return gone,
        }
    }
}

/// Relays one event; `true` when the page's session is gone.
fn relay_event(event: &CdpEvent, relay: &mut Relay, outbox: &mut Vec<Value>) -> bool {
    match relay.read_event(event) {
        ToFrontend::Forward(message) => outbox.push(message),
        ToFrontend::Skip => {}
        ToFrontend::PageGone => return true,
    }
    false
}

/// Writes `outbox` to `sink` and flushes it (also the CDP proxy's, RB-030-06).
pub(crate) async fn flush<S>(sink: &mut S, outbox: &mut Vec<Value>) -> Result<(), S::Error>
where
    S: futures_util::Sink<Message> + Unpin,
{
    for message in outbox.drain(..) {
        sink.feed(Message::text(message.to_string())).await?;
    }
    sink.flush().await
}

#[cfg(test)]
#[path = "endpoint_tests.rs"]
mod tests;
