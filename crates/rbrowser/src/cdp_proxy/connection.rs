//! One client's WebSocket: a flat session of its own on the pipe, [`rb_cdp::ClientSession`] in
//! between. `/devtools/browser/<id>` attaches a browser session (`Target.attachToBrowserTarget`),
//! `/devtools/page/<targetId>` the page itself - after the filter said the page is visible.
//! Calls go to the pipe in the client's order ([`send_ordered`]); a call naming a `targetId`
//! asks the browser for that target first and is refused when it is hidden or unknown. When
//! the client leaves, its sessions are detached and the app re-applies the emulation of every
//! view it attached; a rotated token ends the connection at once.

use std::sync::Arc;

use futures_util::{SinkExt as _, StreamExt as _};
use rb_cdp::proxy::Endpoint;
use rb_cdp::session::Call;
use rb_cdp::target_filter::hides_info;
use rb_cdp::{CallError, ClientSession, FromClient, ToClient};
use rb_engine::EngineError;
use serde_json::{Value, json};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::broadcast::error::{RecvError, TryRecvError};
use tokio::sync::{broadcast, mpsc};
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::protocol::CloseFrame;
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;

use super::server::ProxyServer;
use crate::cdp::{CdpEvent, CdpRouter};
use crate::devtools::endpoint::{flush, send_ordered};

type Reply = (Call, Result<Value, EngineError>);

/// Serves `ws` for `endpoint` until either side ends or the token is rotated.
pub async fn run<S>(ws: WebSocketStream<S>, endpoint: Endpoint, server: Arc<ProxyServer>)
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let router = server.router.clone();
    let filter = server.hooks.filter;
    let mut events = router.subscribe_raw();
    let mut revoked = server.revocations();
    let (mut sink, mut stream) = ws.split();
    let mut client = match open(&router, filter, &endpoint).await {
        Ok(client) => client,
        Err(reason) => {
            let close = CloseFrame {
                code: CloseCode::Policy,
                reason: reason.into(),
            };
            let _ = sink.send(Message::Close(Some(close))).await;
            return;
        }
    };
    let scale = server.hooks.scale.clone();
    let (replies, mut answered) = mpsc::unbounded_channel::<Reply>();
    let mut outbox = Vec::new();
    let mut gone = false;
    loop {
        tokio::select! {
            changed = revoked.changed() => {
                // A rotated token or a stopped proxy: this connection's proof is void.
                let _ = changed;
                break;
            }
            frame = stream.next() => match frame {
                Some(Ok(Message::Text(text))) => {
                    match client.read_message(text.as_str(), scale.as_ref()) {
                        FromClient::Call(call) => {
                            if let Some(reply) = dispatch(&router, filter, call, &replies).await {
                                gone |= drain(&mut events, &mut client, &router, filter, &mut outbox);
                                outbox.push(client.answer(&reply.0, call_result(reply.1), filter));
                            }
                        }
                        FromClient::Answer(answer) => outbox.push(answer),
                        FromClient::Drop => {}
                    }
                }
                Some(Ok(Message::Close(_)) | Err(_)) | None => break,
                Some(Ok(_)) => {}
            },
            Some((call, reply)) = answered.recv() => {
                // Every event the browser sent before this answer goes first.
                gone |= drain(&mut events, &mut client, &router, filter, &mut outbox);
                outbox.push(client.answer(&call, call_result(reply), filter));
            }
            event = events.recv() => match event {
                Ok(event) => gone |= relay(&event, &mut client, &router, filter, &mut outbox),
                Err(RecvError::Lagged(missed)) => {
                    tracing::warn!(missed, "a CDP proxy client fell behind; events were dropped");
                }
                Err(RecvError::Closed) => break,
            },
        }
        if flush(&mut sink, &mut outbox).await.is_err() || gone {
            break;
        }
    }
    let _ = sink.close().await;
    close(&router, &client).await;
    (server.hooks.on_end)(client.touched());
}

/// The client's root session for `endpoint`; the reason when there is none.
async fn open(
    router: &Arc<CdpRouter>,
    filter: &dyn rb_cdp::TargetFilter,
    endpoint: &Endpoint,
) -> Result<ClientSession, &'static str> {
    let attached = match endpoint {
        Endpoint::Browser => router
            .call("Target.attachToBrowserTarget", json!({}), None)
            .await
            .map_err(|_| "no browser session")?,
        Endpoint::Page(target) => {
            if !visible(router, filter, target).await {
                return Err("No target with given id found");
            }
            router
                .call(
                    "Target.attachToTarget",
                    json!({ "targetId": target, "flatten": true }),
                    None,
                )
                .await
                .map_err(|_| "the page could not be attached")?
        }
        Endpoint::Version | Endpoint::List | Endpoint::Protocol => return Err("no socket"),
    };
    let session = attached
        .get("sessionId")
        .and_then(Value::as_str)
        .ok_or("no session")?
        .to_owned();
    Ok(match endpoint {
        Endpoint::Page(target) => ClientSession::page(session, target.clone()),
        _ => ClientSession::browser(session),
    })
}

/// Whether `target` exists and no rule hides it.
async fn visible(router: &CdpRouter, filter: &dyn rb_cdp::TargetFilter, target: &str) -> bool {
    match router
        .call("Target.getTargetInfo", json!({ "targetId": target }), None)
        .await
    {
        Ok(reply) => reply
            .get("targetInfo")
            .is_some_and(|info| !hides_info(filter, info)),
        Err(_) => false,
    }
}

/// Sends `call` now, or - when it names a target - checks the target first in a task of its
/// own. The answer comes back at once or later through `replies`.
async fn dispatch(
    router: &Arc<CdpRouter>,
    filter: &'static dyn rb_cdp::TargetFilter,
    call: Call,
    replies: &mpsc::UnboundedSender<Reply>,
) -> Option<Reply> {
    let Some(target) = call.target_check.clone() else {
        let (method, params, session) = (
            call.method.clone(),
            call.params.clone(),
            call.session.clone(),
        );
        return send_ordered(router, method, params, Some(session), call, replies).await;
    };
    let router = router.clone();
    let replies = replies.clone();
    tokio::spawn(async move {
        let reply = if visible(&router, filter, &target).await {
            router
                .call(&call.method, call.params.clone(), Some(&call.session))
                .await
        } else {
            Err(no_target())
        };
        let _ = replies.send((call, reply));
    });
    None
}

/// The error a hidden or unknown target is answered with.
fn no_target() -> EngineError {
    EngineError::Cdp {
        method: String::new(),
        code: rb_cdp::session::INVALID_PARAMS,
        message: rb_cdp::session::NO_TARGET.to_owned(),
    }
}

fn call_result(reply: Result<Value, EngineError>) -> Result<Value, CallError> {
    reply.map_err(|error| match error {
        EngineError::Cdp { code, message, .. } => CallError { code, message },
        other => CallError {
            code: 0,
            message: other.to_string(),
        },
    })
}

/// Takes the events already received; `true` when the client's session is gone.
fn drain(
    events: &mut broadcast::Receiver<Arc<CdpEvent>>,
    client: &mut ClientSession,
    router: &Arc<CdpRouter>,
    filter: &dyn rb_cdp::TargetFilter,
    outbox: &mut Vec<Value>,
) -> bool {
    let mut gone = false;
    loop {
        match events.try_recv() {
            Ok(event) => gone |= relay(&event, client, router, filter, outbox),
            Err(TryRecvError::Lagged(missed)) => {
                tracing::warn!(
                    missed,
                    "a CDP proxy client fell behind; events were dropped"
                );
            }
            Err(TryRecvError::Empty | TryRecvError::Closed) => return gone,
        }
    }
}

/// Relays one event; `true` when the client's session is gone.
fn relay(
    event: &CdpEvent,
    client: &mut ClientSession,
    router: &Arc<CdpRouter>,
    filter: &dyn rb_cdp::TargetFilter,
    outbox: &mut Vec<Value>,
) -> bool {
    match client.read_event(
        &event.method,
        &event.params,
        event.session_id.as_deref(),
        filter,
    ) {
        ToClient::Forward(message) => outbox.push(message),
        ToClient::Skip => {}
        ToClient::DetachHidden { on, session } => {
            let router = router.clone();
            tokio::spawn(async move {
                let detached = router
                    .call(
                        "Target.detachFromTarget",
                        json!({ "sessionId": session }),
                        Some(&on),
                    )
                    .await;
                if let Err(error) = detached {
                    tracing::debug!(%error, "a hidden target's session was not detached");
                }
            });
        }
        ToClient::Gone => return true,
    }
    false
}

/// Detaches the client's sessions, children first.
async fn close(router: &CdpRouter, client: &ClientSession) {
    for child in client.children() {
        let _ = router
            .call(
                "Target.detachFromTarget",
                json!({ "sessionId": child }),
                Some(client.root()),
            )
            .await;
    }
    let _ = router
        .call(
            "Target.detachFromTarget",
            json!({ "sessionId": client.root() }),
            None,
        )
        .await;
}
