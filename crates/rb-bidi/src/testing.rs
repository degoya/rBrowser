//! [`FakeFirefox`]: a WebDriver BiDi remote end in memory, over a real WebSocket on 127.0.0.1,
//! that answers the commands rBrowser sends the way Firefox does (protocol fixtures from the
//! RB-000-06 spike) and records them; [`FakeLaunch`] hands its endpoint to the engine in place
//! of a Firefox process. Tests make it refuse or ignore a method, answer the agent, push events
//! and drop the connection like a crash.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

use crate::client::lock;
use crate::error::BidiError;
use crate::firefox::{Launch, Launched};

/// The PNG signature, base64: what fake screenshots answer.
pub const FAKE_PNG: &str = "iVBORw0KGgo=";

/// A fake Firefox.
#[derive(Clone)]
pub struct FakeFirefox {
    endpoint: String,
    state: Arc<State>,
}

#[derive(Default)]
struct State {
    commands: Mutex<Vec<(String, Value)>>,
    peers: Mutex<Vec<mpsc::UnboundedSender<Option<String>>>>,
    silent: Mutex<HashSet<String>>,
    refused: Mutex<HashMap<String, String>>,
    agent: Mutex<VecDeque<Value>>,
    history: Mutex<HashMap<String, (Vec<String>, usize)>>,
    viewport: Mutex<HashMap<String, (u64, u64, f64)>>,
    version: Mutex<String>,
    next: Mutex<u32>,
    redirects: Mutex<HashMap<String, String>>,
}

impl FakeFirefox {
    /// Listens on 127.0.0.1 at a free port.
    pub async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("a free port on 127.0.0.1");
        let endpoint = format!(
            "ws://{}/session",
            listener.local_addr().expect("a bound address")
        );
        let state = Arc::new(State::default());
        *lock(&state.version) = "159.0.1".to_owned();
        let accepting = state.clone();
        tokio::spawn(async move {
            while let Ok((stream, _)) = listener.accept().await {
                let state = accepting.clone();
                tokio::spawn(async move {
                    if let Ok(ws) = tokio_tungstenite::accept_async(stream).await {
                        serve(ws, state).await;
                    }
                });
            }
        });
        Self { endpoint, state }
    }

    /// The session endpoint.
    pub fn endpoint(&self) -> String {
        self.endpoint.clone()
    }

    /// A launcher that hands out this endpoint.
    pub fn launcher(&self) -> Arc<FakeLaunch> {
        Arc::new(FakeLaunch {
            endpoint: self.endpoint(),
        })
    }

    /// Every command so far: method and params, in order.
    pub fn commands(&self) -> Vec<(String, Value)> {
        lock(&self.state.commands).clone()
    }

    /// The params of every `method` call so far.
    pub fn calls(&self, method: &str) -> Vec<Value> {
        lock(&self.state.commands)
            .iter()
            .filter(|(name, _)| name == method)
            .map(|(_, params)| params.clone())
            .collect()
    }

    /// `method` is never answered (like `setViewport` under fractional scaling).
    pub fn never_answer(&self, method: &str) {
        lock(&self.state.silent).insert(method.to_owned());
    }

    /// `method` is answered with the error `error` (`unknown command`).
    pub fn refuse(&self, method: &str, error: &str) {
        lock(&self.state.refused).insert(method.to_owned(), error.to_owned());
    }

    /// The next agent command answers `result` (a `CommandResult` as JSON); without one queued,
    /// `resolve` answers `ok` at (10, 20) and everything else `done`.
    pub fn answer_agent(&self, result: Value) {
        lock(&self.state.agent).push_back(result);
    }

    /// A navigation to `from` ends at `to`, as a server's redirect does.
    pub fn redirect(&self, from: &str, to: &str) {
        lock(&self.state.redirects).insert(from.to_owned(), to.to_owned());
    }

    /// The Firefox version `session.new` reports.
    pub fn set_version(&self, version: &str) {
        *lock(&self.state.version) = version.to_owned();
    }

    /// Sends the event `method` to every connection.
    pub fn emit(&self, method: &str, params: Value) {
        let frame = json!({ "type": "event", "method": method, "params": params }).to_string();
        lock(&self.state.peers).retain(|peer| peer.send(Some(frame.clone())).is_ok());
    }

    /// The agent of `channel` in `context` sends `text`.
    pub fn agent_says(&self, channel: &str, context: &str, text: &str) {
        self.emit(
            "script.message",
            json!({
                "channel": channel,
                "data": { "type": "string", "value": text },
                "source": { "realm": "sandbox-realm", "context": context },
            }),
        );
    }

    /// Ends every connection, as a crash does.
    pub fn crash(&self) {
        for peer in lock(&self.state.peers).drain(..) {
            // A peer already gone is fine.
            let _gone = peer.send(None);
        }
    }
}

/// Launches nothing: hands out a fake's endpoint.
pub struct FakeLaunch {
    endpoint: String,
}

#[async_trait]
impl Launch for FakeLaunch {
    async fn launch(&self) -> Result<Launched, BidiError> {
        Ok(Launched {
            endpoint: self.endpoint.clone(),
            process: None,
        })
    }
}

async fn serve<S>(ws: tokio_tungstenite::WebSocketStream<S>, state: Arc<State>)
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    let (mut sink, mut source) = ws.split();
    let (peer, mut outbox) = mpsc::unbounded_channel::<Option<String>>();
    lock(&state.peers).push(peer.clone());
    loop {
        tokio::select! {
            frame = outbox.recv() => match frame {
                Some(Some(text)) => {
                    if sink.send(Message::text(text)).await.is_err() {
                        return;
                    }
                }
                _ => {
                    let _closed = sink.close().await;
                    return;
                }
            },
            message = source.next() => match message {
                Some(Ok(Message::Text(text))) => {
                    let Ok(command) = serde_json::from_str::<Value>(text.as_str()) else {
                        continue;
                    };
                    for frame in respond(&state, &command) {
                        // The outbox keeps the order: events before their answer.
                        let _queued = peer.send(Some(frame));
                    }
                }
                Some(Ok(Message::Close(_))) | None | Some(Err(_)) => return,
                Some(Ok(_)) => {}
            },
        }
    }
}

fn respond(state: &State, command: &Value) -> Vec<String> {
    let id = command["id"].clone();
    let method = command["method"].as_str().unwrap_or_default().to_owned();
    let params = command["params"].clone();
    lock(&state.commands).push((method.clone(), params.clone()));
    if lock(&state.silent).contains(&method) {
        return Vec::new();
    }
    if let Some(error) = lock(&state.refused).get(&method) {
        return vec![
            json!({ "type": "error", "id": id, "error": error, "message": method }).to_string(),
        ];
    }
    let context = params["context"].as_str().unwrap_or_default().to_owned();
    let mut events = Vec::new();
    let loaded = |context: &str, url: &str| {
        json!({ "type": "event", "method": "browsingContext.domContentLoaded",
                "params": { "context": context, "url": url, "navigation": "n", "timestamp": 0 } })
        .to_string()
    };
    let result = match method.as_str() {
        "session.new" => json!({
            "sessionId": "fake-session",
            "capabilities": { "browserName": "firefox", "browserVersion": *lock(&state.version) },
        }),
        "browsingContext.getTree" => tree(state, params["root"].as_str()),
        "browser.createUserContext" => {
            let mut next = lock(&state.next);
            *next += 1;
            json!({ "userContext": format!("uc-{next}") })
        }
        "browsingContext.create" => {
            let mut next = lock(&state.next);
            *next += 1;
            let context = format!("ctx-{next}");
            lock(&state.history).insert(context.clone(), (vec!["about:blank".to_owned()], 0));
            json!({ "context": context })
        }
        "browsingContext.navigate" => {
            let url = params["url"].as_str().unwrap_or_default().to_owned();
            let url = lock(&state.redirects).get(&url).cloned().unwrap_or(url);
            if let Some((entries, at)) = lock(&state.history).get_mut(&context) {
                entries.truncate(*at + 1);
                entries.push(url.clone());
                *at = entries.len() - 1;
            }
            events.push(loaded(&context, &url));
            json!({ "navigation": "n", "url": url })
        }
        "browsingContext.reload" => {
            if let Some(url) = current(state, &context) {
                events.push(loaded(&context, &url));
            }
            json!({ "navigation": "n", "url": current(state, &context) })
        }
        "browsingContext.traverseHistory" => {
            let delta = params["delta"].as_i64().unwrap_or(0);
            let moved = lock(&state.history)
                .get_mut(&context)
                .and_then(|(entries, at)| {
                    let to = usize::try_from(i64::try_from(*at).ok()? + delta).ok()?;
                    let url = entries.get(to)?.clone();
                    *at = to;
                    Some(url)
                });
            if let Some(url) = moved {
                events.push(loaded(&context, &url));
            }
            json!({})
        }
        "browsingContext.close" => {
            lock(&state.history).remove(&context);
            events.push(
                json!({ "type": "event", "method": "browsingContext.contextDestroyed",
                        "params": { "context": context, "parent": null, "url": "about:blank", "children": [] } })
                .to_string(),
            );
            json!({})
        }
        "browsingContext.setViewport" => {
            let size = &params["viewport"];
            lock(&state.viewport).insert(
                context.clone(),
                (
                    size["width"].as_u64().unwrap_or(0),
                    size["height"].as_u64().unwrap_or(0),
                    params["devicePixelRatio"].as_f64().unwrap_or(1.0),
                ),
            );
            json!({})
        }
        "browsingContext.captureScreenshot" => json!({ "data": FAKE_PNG }),
        "script.addPreloadScript" => json!({ "script": "preload-1" }),
        "script.evaluate" => evaluate(state, &params),
        _ => json!({}),
    };
    events.push(json!({ "type": "success", "id": id, "result": result }).to_string());
    events
}

fn tree(state: &State, root: Option<&str>) -> Value {
    let entry = |context: &str| {
        json!({ "context": context, "clientWindow": format!("window-{context}"),
                "url": current(state, context).unwrap_or_default(), "children": [], "parent": null })
    };
    match root {
        Some(root) => json!({ "contexts": [entry(root)] }),
        None => json!({ "contexts": [{ "context": "initial", "clientWindow": "window-initial",
                                      "url": "about:blank", "children": [], "parent": null }] }),
    }
}

fn current(state: &State, context: &str) -> Option<String> {
    lock(&state.history)
        .get(context)
        .and_then(|(entries, at)| entries.get(*at).cloned())
}

fn evaluate(state: &State, params: &Value) -> Value {
    let expression = params["expression"].as_str().unwrap_or_default();
    let context = params["target"]["context"].as_str().unwrap_or_default();
    let text = if expression.contains("__rbAgent.command(") {
        let queued = lock(&state.agent).pop_front();
        let result = queued.unwrap_or_else(|| {
            if expression.contains("\"cmd\":\"resolve\"") {
                json!({ "status": "ok", "x": 10.0, "y": 20.0, "scrolled": false })
            } else {
                json!({ "status": "done" })
            }
        });
        result.to_string()
    } else if expression.contains("innerWidth") {
        let (width, height, dpr) = lock(&state.viewport)
            .get(context)
            .copied()
            .unwrap_or((1280, 800, 1.0));
        json!([width, height, dpr]).to_string()
    } else {
        "ok".to_owned()
    };
    json!({ "type": "success", "realm": "sandbox-realm", "result": { "type": "string", "value": text } })
}
