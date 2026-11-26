//! The proxy end to end over real sockets and the fake browser, K10 among them: no token, no
//! answer and no socket; with the token the shell, the settings window and DevTools are no
//! targets and cannot be attached.

use std::time::Duration;

use futures_util::{SinkExt as _, StreamExt as _};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::HeaderValue;

use super::*;
use crate::cdp::fake::FakeBrowser;

type Ws = WebSocketStream<TcpStream>;

const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

struct Setup {
    fake: Arc<FakeBrowser>,
    server: Arc<ProxyServer>,
    ended: Arc<Mutex<Vec<Vec<String>>>>,
}

fn setup() -> Setup {
    let (fake, router) = FakeBrowser::connect();
    for (id, url) in [
        ("SHELL", crate::shell::SHELL_URL),
        ("SETTINGS", crate::shell::PAGES_URL),
        ("DEVTOOLS", "devtools://devtools/bundled/inspector.html"),
        ("V1", "https://a.test/"),
        ("V2", "https://b.test/"),
    ] {
        fake.add_target(id, url);
    }
    let ended = Arc::new(Mutex::new(Vec::new()));
    let record = ended.clone();
    let hooks = Hooks {
        scale: Arc::new(|target: &str| (target == "V1").then_some(0.5)),
        on_end: Arc::new(move |targets| lock(&record).push(targets)),
        filter: crate::devtools::hidden::filter(),
    };
    let token = ProxyToken::from_text(TOKEN.to_owned());
    let server =
        ProxyServer::start(&Handle::current(), router, None, token, hooks).expect("listens");
    Setup {
        fake,
        server,
        ended,
    }
}

/// `GET path` with `extra` header lines; the status and the body.
async fn get(port: u16, path: &str, extra: &str) -> (u16, String) {
    let mut stream = TcpStream::connect(("127.0.0.1", port))
        .await
        .expect("connects");
    let request = format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n{extra}\r\n");
    stream.write_all(request.as_bytes()).await.expect("writes");
    let mut answer = String::new();
    stream.read_to_string(&mut answer).await.expect("reads");
    let status = answer
        .split(' ')
        .nth(1)
        .and_then(|code| code.parse().ok())
        .expect("a status");
    let body = answer
        .split_once("\r\n\r\n")
        .map(|(_, body)| body.to_owned())
        .unwrap_or_default();
    (status, body)
}

fn bearer() -> String {
    format!("Authorization: Bearer {TOKEN}\r\n")
}

async fn connect(
    url: &str,
    token: Option<&str>,
) -> Result<Ws, tokio_tungstenite::tungstenite::Error> {
    let mut request = url.into_client_request()?;
    if let Some(token) = token {
        request.headers_mut().insert(
            "authorization",
            HeaderValue::from_str(&format!("Bearer {token}")).expect("header"),
        );
    }
    let port = request.uri().port_u16().expect("a port");
    let stream = TcpStream::connect(("127.0.0.1", port))
        .await
        .expect("connects");
    tokio_tungstenite::client_async(request, stream)
        .await
        .map(|(ws, _)| ws)
}

fn browser_url(setup: &Setup) -> String {
    format!(
        "ws://127.0.0.1:{}/devtools/browser/{}",
        setup.server.port(),
        setup.server.browser_id()
    )
}

/// Sends `message` and returns the answer with its id, skipping events.
async fn ask(ws: &mut Ws, message: Value) -> Value {
    let id = message["id"].clone();
    ws.send(Message::text(message.to_string()))
        .await
        .expect("sends");
    loop {
        let frame = tokio::time::timeout(Duration::from_secs(2), ws.next())
            .await
            .expect("an answer in time")
            .expect("open")
            .expect("no error");
        let answer: Value = serde_json::from_str(frame.to_text().expect("text")).expect("json");
        if answer["id"] == id {
            return answer;
        }
    }
}

async fn until(mut done: impl FnMut() -> bool) {
    for _ in 0..400 {
        if done() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("condition not reached");
}

#[tokio::test]
async fn k10_without_a_valid_token_nothing_is_answered_and_no_socket_opens() {
    let setup = setup();
    let port = setup.server.port();
    for path in ["/json/version", "/json/list", "/json/protocol"] {
        assert_eq!(get(port, path, "").await.0, 401, "{path}");
        let wrong = format!("Authorization: Bearer {}\r\n", "f".repeat(64));
        assert_eq!(get(port, path, &wrong).await.0, 401, "{path}");
    }
    assert!(connect(&browser_url(&setup), None).await.is_err());
    assert!(
        connect(&browser_url(&setup), Some(&"f".repeat(64)))
            .await
            .is_err()
    );
    assert!(setup.fake.sent("Target.attachToBrowserTarget").is_empty());
}

#[tokio::test]
async fn k10_with_the_token_the_shell_settings_and_devtools_are_no_targets() {
    let setup = setup();
    let port = setup.server.port();
    let (status, body) = get(port, "/json/list", &bearer()).await;
    assert_eq!(status, 200);
    let list: Value = serde_json::from_str(&body).expect("json");
    let ids: Vec<&str> = list
        .as_array()
        .expect("a list")
        .iter()
        .filter_map(|entry| entry["id"].as_str())
        .collect();
    assert_eq!(ids, ["V1", "V2"]);
    assert_eq!(
        list[0]["webSocketDebuggerUrl"],
        json!(format!("ws://127.0.0.1:{port}/devtools/page/V1"))
    );

    let mut ws = connect(&browser_url(&setup), Some(TOKEN))
        .await
        .expect("connects");
    let targets = ask(&mut ws, json!({"id": 1, "method": "Target.getTargets"})).await;
    let ids: Vec<&str> = targets["result"]["targetInfos"]
        .as_array()
        .expect("infos")
        .iter()
        .filter_map(|info| info["targetId"].as_str())
        .collect();
    assert_eq!(ids, ["V1", "V2"]);
    for guessed in ["SHELL", "SETTINGS", "DEVTOOLS"] {
        let attach = json!({"id": 2, "method": "Target.attachToTarget", "params": {"targetId": guessed, "flatten": true}});
        let refused = ask(&mut ws, attach).await;
        assert_eq!(
            refused["error"]["message"],
            json!("No target with given id found"),
            "{guessed}"
        );
    }
    // A page socket to a hidden target is closed at once.
    let shell = format!("ws://127.0.0.1:{port}/devtools/page/SHELL");
    let mut page = connect(&shell, Some(TOKEN)).await.expect("upgraded");
    let first = tokio::time::timeout(Duration::from_secs(2), page.next())
        .await
        .expect("in time");
    assert!(
        matches!(first, Some(Ok(Message::Close(_))) | None),
        "{first:?}"
    );
    let attached: Vec<Value> = setup.fake.sent("Target.attachToTarget");
    assert!(attached.is_empty(), "{attached:?}");
}

#[tokio::test]
async fn the_version_names_the_browser_socket_and_a_path_token_keeps_its_prefix() {
    let setup = setup();
    let port = setup.server.port();
    let (status, body) = get(port, "/json/version/", &bearer()).await;
    assert_eq!(status, 200);
    let version: Value = serde_json::from_str(&body).expect("json");
    assert_eq!(version["webSocketDebuggerUrl"], json!(browser_url(&setup)));
    assert_eq!(version["Browser"], json!("Chrome/131.0.6778.86"));
    let (status, body) = get(port, &format!("/t/{TOKEN}/json/version"), "").await;
    assert_eq!(status, 200);
    let version: Value = serde_json::from_str(&body).expect("json");
    let url = version["webSocketDebuggerUrl"]
        .as_str()
        .expect("url")
        .to_owned();
    assert!(url.contains(&format!("/t/{TOKEN}/devtools/browser/")));
    assert!(connect(&url, None).await.is_ok());
}

/// A call naming a target is checked in a task of its own: a call after it reaches the pipe
/// while the check still waits - the order the module head of `connection` names (RB-099-18).
#[tokio::test]
async fn a_call_after_a_target_call_is_not_held_up_by_its_check() {
    let setup = setup();
    let mut ws = connect(&browser_url(&setup), Some(TOKEN))
        .await
        .expect("connects");
    setup.fake.hold("Target.getTargetInfo", None);
    let attach = json!({"id": 1, "method": "Target.attachToTarget", "params": {"targetId": "V1", "flatten": true}});
    ws.send(Message::text(attach.to_string()))
        .await
        .expect("sends");
    until(|| !setup.fake.sent("Target.getTargetInfo").is_empty()).await;
    let version = ask(&mut ws, json!({"id": 2, "method": "Browser.getVersion"})).await;
    assert!(version.get("result").is_some(), "{version}");
    assert!(
        setup.fake.sent("Target.attachToTarget").is_empty(),
        "the target call waits for its check"
    );
    setup.fake.release();
    loop {
        let frame = tokio::time::timeout(Duration::from_secs(2), ws.next())
            .await
            .expect("an answer in time")
            .expect("open")
            .expect("no error");
        let answer: Value = serde_json::from_str(frame.to_text().expect("text")).expect("json");
        if answer["id"] == 1 {
            assert!(answer["result"]["sessionId"].is_string(), "{answer}");
            break;
        }
    }
    assert_eq!(setup.fake.sent("Target.attachToTarget").len(), 1);
}

#[tokio::test]
async fn device_metrics_stay_rbrowsers_and_input_is_scaled_to_the_view() {
    let setup = setup();
    let mut ws = connect(&browser_url(&setup), Some(TOKEN))
        .await
        .expect("connects");
    let attach = json!({"id": 1, "method": "Target.attachToTarget", "params": {"targetId": "V1", "flatten": true}});
    let session = ask(&mut ws, attach).await["result"]["sessionId"].clone();
    let metrics = json!({"id": 2, "sessionId": session, "method": "Emulation.setDeviceMetricsOverride", "params": {"width": 800, "height": 600, "deviceScaleFactor": 1, "mobile": false}});
    assert_eq!(ask(&mut ws, metrics).await["result"], json!({}));
    assert!(
        setup
            .fake
            .sent("Emulation.setDeviceMetricsOverride")
            .is_empty()
    );
    let click = json!({"id": 3, "sessionId": session, "method": "Input.dispatchMouseEvent", "params": {"type": "mousePressed", "x": 100, "y": 60, "button": "left", "clickCount": 1}});
    ask(&mut ws, click).await;
    let sent = setup.fake.sent("Input.dispatchMouseEvent");
    assert_eq!(sent[0]["params"]["x"], json!(50.0));
    assert_eq!(sent[0]["params"]["y"], json!(30.0));
    ws.close(None).await.expect("closes");
    until(|| !lock(&setup.ended).is_empty()).await;
    assert_eq!(lock(&setup.ended)[0], ["V1".to_owned()]);
    let detached = setup.fake.sent("Target.detachFromTarget");
    assert!(
        detached
            .iter()
            .any(|message| message["params"]["sessionId"] == session)
    );
}

#[tokio::test]
async fn an_auto_attached_hidden_target_is_detached_without_a_word() {
    let setup = setup();
    let mut ws = connect(&browser_url(&setup), Some(TOKEN))
        .await
        .expect("connects");
    ask(&mut ws, json!({"id": 1, "method": "Target.setAutoAttach", "params": {"autoAttach": true, "waitForDebuggerOnStart": true, "flatten": true}})).await;
    let root = setup.fake.sent("Target.setAutoAttach")[0]["sessionId"].clone();
    setup.fake.emit(json!({"method": "Target.attachedToTarget", "sessionId": root, "params": {"sessionId": "HIDDEN", "targetInfo": {"targetId": "SHELL", "type": "page", "url": crate::shell::SHELL_URL}, "waitingForDebugger": true}}));
    setup.fake.emit(json!({"method": "Target.attachedToTarget", "sessionId": root, "params": {"sessionId": "VISIBLE", "targetInfo": {"targetId": "V2", "type": "page", "url": "https://b.test/"}, "waitingForDebugger": true}}));
    let frame = tokio::time::timeout(Duration::from_secs(2), ws.next())
        .await
        .expect("in time")
        .expect("open")
        .expect("no error");
    let event: Value = serde_json::from_str(frame.to_text().expect("text")).expect("json");
    assert_eq!(event["params"]["sessionId"], json!("VISIBLE"));
    until(|| {
        setup
            .fake
            .sent("Target.detachFromTarget")
            .iter()
            .any(|message| message["params"]["sessionId"] == "HIDDEN")
    })
    .await;
}

#[tokio::test]
async fn a_rotated_token_ends_open_connections_and_only_the_new_one_gets_in() {
    let setup = setup();
    let port = setup.server.port();
    let mut ws = connect(&browser_url(&setup), Some(TOKEN))
        .await
        .expect("connects");
    let fresh = "e".repeat(64);
    setup.server.rotate(ProxyToken::from_text(fresh.clone()));
    let ended = tokio::time::timeout(Duration::from_secs(2), ws.next())
        .await
        .expect("in time");
    assert!(
        matches!(ended, Some(Ok(Message::Close(_)) | Err(_)) | None),
        "{ended:?}"
    );
    assert_eq!(get(port, "/json/version", &bearer()).await.0, 401);
    let new = format!("Authorization: Bearer {fresh}\r\n");
    assert_eq!(get(port, "/json/version", &new).await.0, 200);
    assert_eq!(
        format!("{:?}", setup.server.token()),
        "ProxyToken(<redacted>)"
    );
}

/// Sets its flag when dropped.
struct Flag(Arc<std::sync::atomic::AtomicBool>);

impl Drop for Flag {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

#[tokio::test]
async fn what_the_proxy_keeps_lives_until_its_last_connection_ends() {
    // A flow's view slot: freed only when no connection can judge a target with it any more.
    let Setup { server, fake, .. } = setup();
    let freed = Arc::new(std::sync::atomic::AtomicBool::new(false));
    server.keep_until_dropped(Box::new(Flag(freed.clone())));
    let url = format!(
        "ws://127.0.0.1:{}/devtools/browser/{}",
        server.port(),
        server.browser_id()
    );
    let mut ws = connect(&url, Some(TOKEN)).await.expect("connects");
    let answer = ask(&mut ws, json!({ "id": 1, "method": "Target.getTargets" })).await;
    assert!(answer.get("result").is_some(), "{answer}");
    drop(server);
    // The barrier: the connection still answers, after the drop.
    let answer = ask(&mut ws, json!({ "id": 2, "method": "Target.getTargets" })).await;
    assert!(answer.get("result").is_some(), "{answer}");
    assert!(!freed.load(Ordering::SeqCst), "a connection still runs");
    ws.close(None).await.expect("closes");
    until(|| freed.load(Ordering::SeqCst)).await;
    drop(fake);
}

#[tokio::test]
async fn a_stopped_proxy_listens_no_more() {
    let setup = setup();
    let port = setup.server.port();
    setup.server.stop();
    drop(setup);
    let mut refused = false;
    for _ in 0..200 {
        if TcpStream::connect(("127.0.0.1", port)).await.is_err() {
            refused = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    assert!(refused, "the port still accepts connections");
}

#[tokio::test]
async fn a_foreign_origin_or_host_is_forbidden_even_with_the_token() {
    let setup = setup();
    let port = setup.server.port();
    let origin = format!("{}Origin: https://evil.test\r\n", bearer());
    assert_eq!(get(port, "/json/version", &origin).await.0, 403);
    let mut stream = TcpStream::connect(("127.0.0.1", port))
        .await
        .expect("connects");
    let request = format!(
        "GET /json/version HTTP/1.1\r\nHost: evil.test\r\n{}\r\n",
        bearer()
    );
    stream.write_all(request.as_bytes()).await.expect("writes");
    let mut answer = String::new();
    stream.read_to_string(&mut answer).await.expect("reads");
    assert!(answer.starts_with("HTTP/1.1 403"), "{answer}");
}

/// Every log line, at every level, into a buffer.
#[derive(Clone, Default)]
struct Captured(Arc<Mutex<Vec<u8>>>);

impl std::io::Write for Captured {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        lock(&self.0).extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[tokio::test]
async fn the_token_is_in_no_log_line() {
    let captured = Captured::default();
    let writer = captured.clone();
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .with_writer(move || writer.clone())
        .finish();
    // The test's runtime has one thread: every task of the proxy logs into this subscriber.
    let _guard = tracing::subscriber::set_default(subscriber);
    let setup = setup();
    let port = setup.server.port();
    assert_eq!(
        get(port, &format!("/t/{TOKEN}/json/nothing"), "").await.0,
        404
    );
    assert_eq!(
        get(port, &format!("/t/{TOKEN}x/json/version"), "").await.0,
        401
    );
    let origin = format!("{}Origin: https://evil.test\r\n", bearer());
    assert_eq!(get(port, "/json/version", &origin).await.0, 403);
    assert!(
        connect(&browser_url(&setup), Some(&format!("{TOKEN}0")))
            .await
            .is_err()
    );
    let ws = connect(&browser_url(&setup), Some(TOKEN))
        .await
        .expect("connects");
    drop(ws);
    let fresh = "9".repeat(64);
    setup.server.rotate(ProxyToken::from_text(fresh.clone()));
    let logged = || String::from_utf8_lossy(&lock(&captured.0)).into_owned();
    until(|| logged().contains("refused")).await;
    let log = logged();
    assert!(!log.contains(TOKEN), "{log}");
    assert!(!log.contains(&fresh), "{log}");
}
