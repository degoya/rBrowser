//! [`FakeSafari`]: a W3C WebDriver remote end in memory over real HTTP on 127.0.0.1, answering
//! the commands rBrowser sends the way `safaridriver` does, and recording them. One window per
//! session with a history, a rectangle and Safari's chrome (74 px of toolbar); the page's scripts
//! answer the viewport probe, the agent's boot and commands, and the submit block. Tests make it
//! refuse new sessions (remote automation off, another session), refuse a command, answer the
//! agent, keep windows from getting narrow, and stop like a crashed driver.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::sync::Mutex;

use rb_util::lock;
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;

use crate::agent::{BLOCK_SUBMIT, COMMAND_SCRIPT};
use crate::emulation::VIEWPORT_PROBE;
use crate::protocol::ELEMENT_KEY;

/// The PNG signature, base64: what fake screenshots answer.
pub const FAKE_PNG: &str = "iVBORw0KGgo=";
/// Safari's chrome in the fake: no border, a 74 px toolbar.
pub const CHROME: (i64, i64) = (0, 74);

/// One request the fake took.
#[derive(Clone, Debug, PartialEq)]
pub struct Request {
    /// `GET`, `POST`, `DELETE`.
    pub method: String,
    /// The path.
    pub path: String,
    /// The JSON body (`Null` without one).
    pub body: Value,
}

#[derive(Clone, Debug, Default)]
struct Window {
    history: Vec<String>,
    index: usize,
    rect: (i64, i64, i64, i64),
    agent: bool,
}

#[derive(Default)]
struct State {
    requests: Mutex<Vec<Request>>,
    windows: Mutex<HashMap<String, Window>>,
    next: Mutex<u32>,
    refuse_sessions: Mutex<Option<String>>,
    max_sessions: Mutex<Option<usize>>,
    refused: Mutex<HashMap<String, (String, String)>>,
    agent: Mutex<VecDeque<Value>>,
    missing: Mutex<HashSet<String>>,
    min_width: Mutex<i64>,
    screen: Mutex<Option<(i64, i64)>>,
    stopped: std::sync::atomic::AtomicBool,
    ran: Mutex<Vec<String>>,
}

/// A fake `safaridriver`.
#[derive(Clone)]
pub struct FakeSafari {
    port: u16,
    state: Arc<State>,
    task: Arc<Mutex<Option<JoinHandle<()>>>>,
}

impl FakeSafari {
    /// Listens on 127.0.0.1 at a free port.
    pub async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("a free port on 127.0.0.1");
        let port = listener.local_addr().expect("a bound address").port();
        let state = Arc::new(State::default());
        let serving = state.clone();
        let task = tokio::spawn(async move {
            while let Ok((stream, _)) = listener.accept().await {
                let state = serving.clone();
                tokio::spawn(async move { serve(stream, &state).await });
            }
        });
        Self {
            port,
            state,
            task: Arc::new(Mutex::new(Some(task))),
        }
    }

    /// Its port.
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Every request so far.
    pub fn requests(&self) -> Vec<Request> {
        lock(&self.state.requests).clone()
    }

    /// The requests whose path ends with `suffix`.
    pub fn requests_to(&self, suffix: &str) -> Vec<Request> {
        self.requests()
            .into_iter()
            .filter(|request| request.path.ends_with(suffix))
            .collect()
    }

    /// The sessions open now.
    pub fn sessions(&self) -> usize {
        lock(&self.state.windows).len()
    }

    /// The URL session `id` shows.
    pub fn url_of(&self, id: &str) -> Option<String> {
        lock(&self.state.windows)
            .get(id)
            .and_then(|window| window.history.get(window.index).cloned())
    }

    /// The window rectangle of session `id`: x, y, width, height.
    pub fn rect_of(&self, id: &str) -> Option<(i64, i64, i64, i64)> {
        lock(&self.state.windows).get(id).map(|window| window.rect)
    }

    /// New sessions are refused with `message` (`session not created`).
    pub fn refuse_sessions(&self, message: &str) {
        *lock(&self.state.refuse_sessions) = Some(message.to_owned());
    }

    /// New sessions are taken again.
    pub fn accept_sessions(&self) {
        *lock(&self.state.refuse_sessions) = None;
    }

    /// Session `id`'s window was closed in Safari: the driver no longer knows the session.
    pub fn close_window(&self, id: &str) {
        lock(&self.state.windows).remove(id);
    }

    /// The ids of the sessions open now.
    pub fn session_ids(&self) -> Vec<String> {
        let mut ids: Vec<String> = lock(&self.state.windows).keys().cloned().collect();
        ids.sort();
        ids
    }

    /// The agent commands (`cmd`) the page's agent ran, in order (not those before its boot).
    pub fn agent_commands(&self) -> Vec<String> {
        lock(&self.state.ran).clone()
    }

    /// The bodies of `POST /actions`, in order.
    pub fn actions(&self) -> Vec<Value> {
        self.requests_to("/actions")
            .into_iter()
            .filter(|request| request.method == "POST")
            .map(|request| request.body["actions"].clone())
            .collect()
    }

    /// At most `count` sessions at once; the next is refused as Safari does.
    pub fn max_sessions(&self, count: usize) {
        *lock(&self.state.max_sessions) = Some(count);
    }

    /// Requests whose path ends with `suffix` are refused with WebDriver's `error`.
    pub fn refuse(&self, suffix: &str, error: &str) {
        lock(&self.state.refused).insert(
            suffix.to_owned(),
            (error.to_owned(), format!("{suffix} refused")),
        );
    }

    /// The agent answers `answer` to its next command (in order; `done` or a point after).
    pub fn answer_agent(&self, answer: Value) {
        lock(&self.state.agent).push_back(answer);
    }

    /// No element matches `selector`.
    pub fn hide(&self, selector: &str) {
        lock(&self.state.missing).insert(selector.to_owned());
    }

    /// Windows are never narrower than `width` (Safari's narrowest window).
    pub fn min_width(&self, width: i64) {
        *lock(&self.state.min_width) = width;
    }

    /// New windows show a viewport of `width` x `height`, as an iPhone's screen does (iOS,
    /// RB-095-04).
    pub fn screen(&self, width: i64, height: i64) {
        *lock(&self.state.screen) = Some((width, height));
    }

    /// Stops listening and drops every session: the driver crashed.
    pub fn stop(&self) {
        self.state
            .stopped
            .store(true, std::sync::atomic::Ordering::Release);
        if let Some(task) = lock(&self.task).take() {
            task.abort();
        }
        lock(&self.state.windows).clear();
    }
}

async fn serve(mut stream: TcpStream, state: &State) {
    if state.stopped.load(std::sync::atomic::Ordering::Acquire) {
        return;
    }
    let Some(request) = read_request(&mut stream).await else {
        return;
    };
    lock(&state.requests).push(request.clone());
    let (status, value) = answer(state, &request);
    let body = json!({ "value": value }).to_string();
    let head = format!(
        "HTTP/1.1 {status} X\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(head.as_bytes()).await;
    let _ = stream.write_all(body.as_bytes()).await;
    let _ = stream.shutdown().await;
}

async fn read_request(stream: &mut TcpStream) -> Option<Request> {
    let mut raw = Vec::new();
    let mut buffer = vec![0_u8; 64 * 1024];
    let end = loop {
        let read = stream.read(&mut buffer).await.ok()?;
        if read == 0 {
            return None;
        }
        raw.extend_from_slice(&buffer[..read]);
        if let Some(end) = raw.windows(4).position(|window| window == b"\r\n\r\n") {
            break end;
        }
    };
    let head = String::from_utf8_lossy(&raw[..end]).into_owned();
    let mut lines = head.split("\r\n");
    let mut first = lines.next()?.split(' ');
    let method = first.next()?.to_owned();
    let path = first.next()?.to_owned();
    let length: usize = lines
        .filter_map(|line| line.split_once(':'))
        .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
        .and_then(|(_, value)| value.trim().parse().ok())
        .unwrap_or(0);
    while raw.len() < end + 4 + length {
        let read = stream.read(&mut buffer).await.ok()?;
        if read == 0 {
            return None;
        }
        raw.extend_from_slice(&buffer[..read]);
    }
    let body = &raw[end + 4..end + 4 + length];
    let body = if body.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(body).ok()?
    };
    Some(Request { method, path, body })
}

fn error(status: u16, error: &str, message: &str) -> (u16, Value) {
    (
        status,
        json!({ "error": error, "message": message, "stacktrace": "" }),
    )
}

fn answer(state: &State, request: &Request) -> (u16, Value) {
    if let Some((code, message)) = lock(&state.refused)
        .iter()
        .find(|(suffix, _)| request.path.ends_with(suffix.as_str()))
        .map(|(_, refused)| refused.clone())
    {
        return error(500, &code, &message);
    }
    let parts: Vec<&str> = request.path.trim_start_matches('/').split('/').collect();
    match (request.method.as_str(), parts.as_slice()) {
        ("GET", ["status"]) => (200, json!({ "ready": true, "message": "" })),
        ("POST", ["session"]) => new_session(state),
        (_, ["session", id, rest @ ..]) => {
            let id = (*id).to_owned();
            let mut windows = lock(&state.windows);
            let Some(window) = windows.get_mut(&id) else {
                return error(404, "invalid session id", "no such session");
            };
            let min_width = *lock(&state.min_width);
            let answer = in_session(state, window, request, rest, min_width);
            if request.method == "DELETE" && rest.is_empty() {
                windows.remove(&id);
            }
            answer
        }
        _ => error(404, "unknown command", "no such route"),
    }
}

fn new_session(state: &State) -> (u16, Value) {
    if let Some(message) = lock(&state.refuse_sessions).clone() {
        return error(500, "session not created", &message);
    }
    let mut windows = lock(&state.windows);
    if lock(&state.max_sessions).is_some_and(|max| windows.len() >= max) {
        return error(
            500,
            "session not created",
            "Could not create a session: The Safari instance is already paired with another WebDriver session.",
        );
    }
    let id = {
        let mut next = lock(&state.next);
        *next += 1;
        format!("fake-session-{next}")
    };
    let screen = *lock(&state.screen);
    let (width, height) = screen.unwrap_or((1024, 768));
    windows.insert(
        id.clone(),
        Window {
            history: vec!["about:blank".to_owned()],
            index: 0,
            rect: (0, 0, width + CHROME.0, height + CHROME.1),
            agent: false,
        },
    );
    let capabilities = json!({ "browserName": "Safari", "browserVersion": "26.1" });
    (
        200,
        json!({ "sessionId": id, "capabilities": capabilities }),
    )
}

fn in_session(
    state: &State,
    window: &mut Window,
    request: &Request,
    rest: &[&str],
    min_width: i64,
) -> (u16, Value) {
    let body = &request.body;
    match (request.method.as_str(), rest) {
        ("DELETE", []) => (200, Value::Null),
        ("POST", ["url"]) => {
            let url = body.get("url").and_then(Value::as_str).unwrap_or_default();
            window.history.truncate(window.index + 1);
            window.history.push(url.to_owned());
            window.index = window.history.len() - 1;
            window.agent = false;
            (200, Value::Null)
        }
        ("GET", ["url"]) => (200, json!(window.history[window.index])),
        ("POST", ["back"]) => {
            window.index = window.index.saturating_sub(1);
            window.agent = false;
            (200, Value::Null)
        }
        ("POST", ["forward"]) => {
            window.index = (window.index + 1).min(window.history.len() - 1);
            window.agent = false;
            (200, Value::Null)
        }
        ("POST", ["refresh"]) => {
            window.agent = false;
            (200, Value::Null)
        }
        ("POST", ["execute", "sync"]) => (200, script(state, window, body)),
        ("POST", ["actions"]) | ("DELETE", ["actions"]) => (200, Value::Null),
        ("POST", ["window", "rect"]) => {
            let field =
                |key: &str, current: i64| body.get(key).and_then(Value::as_i64).unwrap_or(current);
            let (x, y, width, height) = window.rect;
            window.rect = (
                field("x", x),
                field("y", y),
                field("width", width).max(min_width),
                field("height", height),
            );
            let (x, y, width, height) = window.rect;
            (
                200,
                json!({ "x": x, "y": y, "width": width, "height": height }),
            )
        }
        ("GET", ["window"]) => (200, json!("fake-window")),
        ("POST", ["window"]) => (200, Value::Null),
        ("GET", ["screenshot"]) | ("GET", ["element", _, "screenshot"]) => (200, json!(FAKE_PNG)),
        ("POST", ["element"]) => {
            let selector = body
                .get("value")
                .and_then(Value::as_str)
                .unwrap_or_default();
            if lock(&state.missing).contains(selector) {
                error(404, "no such element", "Unable to locate an element")
            } else {
                (200, json!({ ELEMENT_KEY: "fake-node-1" }))
            }
        }
        _ => error(404, "unknown command", "no such route"),
    }
}

/// What the page answers a script: the probe, the agent's boot and commands, the submit block.
fn script(state: &State, window: &mut Window, body: &Value) -> Value {
    let source = body
        .get("script")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let args = body.get("args").cloned().unwrap_or(Value::Null);
    if source == VIEWPORT_PROBE {
        let (_, _, width, height) = window.rect;
        return json!([CHROME.0, CHROME.1, width - CHROME.0, height - CHROME.1]);
    }
    if source == COMMAND_SCRIPT {
        if !window.agent {
            return Value::Null;
        }
        let command = args.get(1).cloned().unwrap_or(Value::Null);
        let cmd = command
            .get("cmd")
            .and_then(Value::as_str)
            .unwrap_or_default();
        lock(&state.ran).push(cmd.to_owned());
        let resolve = cmd == "resolve";
        let queued = lock(&state.agent).pop_front();
        let result = queued.unwrap_or_else(|| {
            if resolve {
                json!({ "status": "ok", "x": 10.0, "y": 20.0, "scrolled": false })
            } else {
                json!({ "status": "done" })
            }
        });
        return json!(result.to_string());
    }
    if source == BLOCK_SUBMIT {
        return json!("ok");
    }
    if source.contains("Object.defineProperty(globalThis, name") {
        window.agent = true;
        return json!(true);
    }
    Value::Null
}
