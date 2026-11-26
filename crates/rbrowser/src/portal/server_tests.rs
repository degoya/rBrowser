//! The portal end to end over real sockets on 127.0.0.1 (an explicit interface): pairing once,
//! the 403s, the proxy with the agent, a device's socket, disconnect.

use std::sync::atomic::{AtomicUsize, Ordering};

use futures_util::StreamExt as _;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::HeaderValue;

use super::*;
use crate::portal::inject::AGENT_TAG;
use crate::portal::pairing::COOKIE_NAME;

const AGENT: &str = "/*portal agent*/";

pub(super) fn start(ttl: Duration, max_devices: usize) -> (Arc<PortalServer>, Arc<AtomicUsize>) {
    let changes = Arc::new(AtomicUsize::new(0));
    let counted = changes.clone();
    let server = PortalServer::start(
        &Handle::current(),
        Options {
            ip: Ipv4Addr::LOCALHOST,
            port: None,
            pairing_ttl: ttl,
            max_devices,
            agent: Arc::from(AGENT),
            on_change: Arc::new(move || {
                counted.fetch_add(1, Ordering::SeqCst);
            }),
        },
    )
    .expect("listens");
    (server, changes)
}

/// Sends `request` as it is; the whole answer.
pub(super) async fn raw(server: &PortalServer, request: &str) -> String {
    let mut stream = TcpStream::connect(server.address())
        .await
        .expect("connects");
    stream.write_all(request.as_bytes()).await.expect("writes");
    let mut answer = Vec::new();
    let _ = tokio::time::timeout(Duration::from_secs(5), stream.read_to_end(&mut answer)).await;
    String::from_utf8_lossy(&answer).into_owned()
}

/// `GET path` with the portal's host and `extra` header lines.
pub(super) async fn get(server: &PortalServer, path: &str, extra: &str) -> String {
    let request = format!(
        "GET {path} HTTP/1.1\r\nHost: {}\r\nUser-Agent: TestPhone\r\n{extra}\r\n",
        server.address()
    );
    raw(server, &request).await
}

pub(super) fn status(answer: &str) -> u16 {
    answer
        .split(' ')
        .nth(1)
        .and_then(|code| code.parse().ok())
        .unwrap_or(0)
}

/// Pairs a device; its cookie header line.
pub(super) async fn pair(server: &PortalServer) -> String {
    let pairing = server.issue_pairing().expect("random");
    let path = pairing.url.trim_start_matches(server.origin());
    let answer = get(server, path, "").await;
    assert_eq!(status(&answer), 200, "{answer}");
    let cookie = answer
        .lines()
        .find_map(|line| line.strip_prefix("Set-Cookie: "))
        .expect("a cookie")
        .split(';')
        .next()
        .expect("a pair")
        .to_owned();
    format!("Cookie: {cookie}\r\n")
}

#[tokio::test]
async fn a_pairing_token_works_once_and_sets_a_strict_cookie() {
    let (server, changes) = start(Duration::from_secs(300), 5);
    let pairing = server.issue_pairing().expect("random");
    assert!(
        pairing
            .url
            .starts_with(&format!("http://{}{PAIR_PATH}?t=", server.address()))
    );
    let path = pairing.url.trim_start_matches(server.origin()).to_owned();
    let first = get(&server, &path, "").await;
    // A `200` with a page that goes on to `/` itself: a scanner app's navigation is cross-site,
    // and the `Strict` cookie would not follow a `303` of that chain (owner test 2026-11-13).
    assert_eq!(status(&first), 200, "{first}");
    assert!(!first.contains("Location:"), "{first}");
    assert!(first.contains("Content-Type: text/html"), "{first}");
    assert!(first.contains("location.replace(\"/\")"), "{first}");
    assert!(
        first.contains("http-equiv=\"refresh\" content=\"0; url=/\""),
        "{first}"
    );
    assert!(first.contains(&format!("Set-Cookie: {COOKIE_NAME}=")));
    assert!(first.contains("HttpOnly; SameSite=Strict"));
    assert!(first.contains("Cache-Control: no-store"));
    assert!(changes.load(Ordering::SeqCst) >= 1);
    let second = get(&server, &path, "").await;
    assert_eq!(status(&second), 403, "{second}");
    assert_eq!(server.devices().len(), 1);
    assert_eq!(server.devices()[0].info.name, "TestPhone");
}

#[tokio::test]
async fn an_expired_token_answers_403() {
    let (server, _) = start(Duration::from_millis(1), 5);
    let pairing = server.issue_pairing().expect("random");
    tokio::time::sleep(Duration::from_millis(20)).await;
    let path = pairing.url.trim_start_matches(server.origin()).to_owned();
    assert_eq!(status(&get(&server, &path, "").await), 403);
    assert!(server.devices().is_empty());
}

#[tokio::test]
async fn without_the_cookie_every_request_answers_403() {
    let (server, _) = start(Duration::from_secs(300), 5);
    for path in ["/", "/app.js", AGENT_PATH, SOCKET_PATH] {
        assert_eq!(status(&get(&server, path, "").await), 403, "{path}");
    }
    let forged = format!("Cookie: {COOKIE_NAME}=00ff\r\n");
    assert_eq!(status(&get(&server, "/", &forged).await), 403);
}

#[tokio::test]
async fn another_host_and_proxy_requests_answer_403() {
    let (server, _) = start(Duration::from_secs(300), 5);
    let cookie = pair(&server).await;
    let other_host = raw(
        &server,
        &format!("GET / HTTP/1.1\r\nHost: evil.example.com\r\n{cookie}\r\n"),
    )
    .await;
    assert_eq!(status(&other_host), 403);
    let absolute = raw(
        &server,
        &format!(
            "GET http://evil.example.com/ HTTP/1.1\r\nHost: {}\r\n{cookie}\r\n",
            server.address()
        ),
    )
    .await;
    assert_eq!(status(&absolute), 403);
    let connect = raw(&server, "CONNECT evil.example.com:443 HTTP/1.1\r\n\r\n").await;
    assert_eq!(status(&connect), 403);
}

#[tokio::test]
async fn a_page_outside_the_project_answers_403() {
    let (server, _) = start(Duration::from_secs(300), 5);
    let cookie = pair(&server).await;
    server.set_page(
        Some("http://evil.example.com/".to_owned()),
        vec!["http://localhost:5173".to_owned()],
    );
    assert_eq!(status(&get(&server, "/", &cookie).await), 403);
    server.set_page(None, vec!["http://localhost:5173".to_owned()]);
    assert_eq!(status(&get(&server, "/", &cookie).await), 403);
}

#[tokio::test]
async fn the_agent_is_served_to_a_paired_device() {
    let (server, _) = start(Duration::from_secs(300), 5);
    let cookie = pair(&server).await;
    let answer = get(&server, AGENT_PATH, &cookie).await;
    assert_eq!(status(&answer), 200);
    assert!(answer.contains("Content-Type: application/javascript"));
    assert!(answer.ends_with(AGENT));
    assert_eq!(
        status(&get(&server, "/__rbrowser/other", &cookie).await),
        404
    );
}

/// An upstream that answers every request with `answer` and hands the requests it saw out.
async fn upstream(answer: &'static str) -> (String, tokio::sync::mpsc::Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("binds");
    let origin = format!("http://{}", listener.local_addr().expect("address"));
    let (seen, requests) = tokio::sync::mpsc::channel(8);
    tokio::spawn(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            let mut head = Vec::new();
            let mut byte = [0u8; 1];
            while head_end(&head).is_none() && stream.read(&mut byte).await.unwrap_or(0) == 1 {
                head.push(byte[0]);
            }
            let _ = seen.send(String::from_utf8_lossy(&head).into_owned()).await;
            let _ = stream.write_all(answer.as_bytes()).await;
            let _ = stream.shutdown().await;
        }
    });
    (origin, requests)
}

#[tokio::test]
async fn an_html_page_comes_through_with_the_agent_and_without_the_cookie() {
    let (origin, mut requests) = upstream(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nTransfer-Encoding: chunked\r\n\r\n\
         d\r\n<head></head>\r\n0\r\n\r\n",
    )
    .await;
    let (server, _) = start(Duration::from_secs(300), 5);
    let cookie = pair(&server).await;
    server.set_page(Some(format!("{origin}/start")), vec![origin.clone()]);
    let answer = get(&server, "/start?x=1", &format!("{cookie}Cookie: app=1\r\n")).await;
    assert_eq!(status(&answer), 200, "{answer}");
    assert!(
        answer.ends_with(&format!("<head>{AGENT_TAG}</head>")),
        "{answer}"
    );
    assert!(answer.contains(&format!("Content-Length: {}", 13 + AGENT_TAG.len())));
    let seen = requests.recv().await.expect("the upstream was asked");
    assert!(seen.starts_with("GET /start?x=1 HTTP/1.1\r\n"));
    assert!(seen.contains(&format!(
        "Host: {}\r\n",
        origin.trim_start_matches("http://")
    )));
    assert!(!seen.contains(COOKIE_NAME), "{seen}");
    assert!(seen.contains("Cookie: app=1\r\n"));
}

#[tokio::test]
async fn a_redirect_on_the_upstream_stays_on_the_portal() {
    let (origin, _requests) = upstream(
        "HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:9/login\r\nContent-Length: 0\r\n\r\n",
    )
    .await;
    // Another origin than the upstream's stays as it is (the rewrite itself: rewrite_tests).
    let (server, _) = start(Duration::from_secs(300), 5);
    let cookie = pair(&server).await;
    server.set_page(Some(origin.clone()), vec![origin.clone()]);
    let answer = get(&server, "/", &cookie).await;
    assert_eq!(status(&answer), 302);
    assert!(answer.contains("Location: http://127.0.0.1:9/login\r\n"));
}

pub(super) async fn socket(server: &PortalServer, cookie: &str) -> WebSocketStream<TcpStream> {
    let stream = TcpStream::connect(server.address())
        .await
        .expect("connects");
    let mut request = format!("ws://{}{SOCKET_PATH}", server.address())
        .into_client_request()
        .expect("a request");
    let value = cookie.trim_start_matches("Cookie: ").trim_end();
    request
        .headers_mut()
        .insert("cookie", HeaderValue::from_str(value).expect("a header"));
    tokio_tungstenite::client_async(request, stream)
        .await
        .expect("upgrades")
        .0
}

async fn next_text(ws: &mut WebSocketStream<TcpStream>) -> Option<String> {
    match tokio::time::timeout(Duration::from_secs(5), ws.next()).await {
        Ok(Some(Ok(Message::Text(text)))) => Some(text.as_str().to_owned()),
        _ => None,
    }
}

#[tokio::test]
async fn a_device_follows_and_disconnect_ends_it() {
    let (server, _) = start(Duration::from_secs(300), 5);
    let cookie = pair(&server).await;
    let allow = vec!["http://localhost:5173".to_owned()];
    server.set_page(Some("http://localhost:5173/a".to_owned()), allow.clone());
    let mut ws = socket(&server, &cookie).await;
    assert_eq!(
        next_text(&mut ws).await.as_deref(),
        Some(r#"{"kind":"navigate","path":"/a"}"#),
        "a new socket first goes where the grid is"
    );
    server.set_page(Some("http://localhost:5173/b?c".to_owned()), allow);
    assert_eq!(
        next_text(&mut ws).await.as_deref(),
        Some(r#"{"kind":"navigate","path":"/b?c"}"#)
    );
    let device = server.devices()[0].info.id;
    assert!(server.devices()[0].connected);
    assert!(server.disconnect(device));
    assert_eq!(next_text(&mut ws).await, None, "the socket closes");
    assert_eq!(status(&get(&server, "/", &cookie).await), 403);
    assert!(server.devices().is_empty());
}

#[tokio::test]
async fn a_stopped_portal_listens_no_more() {
    let (server, _) = start(Duration::from_secs(300), 5);
    let address = server.address();
    server.stop();
    assert!(server.is_stopped());
    // The accept loop hears the stop on its own time: asked again until it is gone.
    let mut refused = false;
    for _ in 0..200 {
        let connected =
            tokio::time::timeout(Duration::from_secs(2), TcpStream::connect(address)).await;
        if !matches!(connected, Ok(Ok(_))) {
            refused = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    assert!(refused, "nobody accepts after the stop");
}

#[test]
fn a_portal_page_escapes_its_text() {
    assert_eq!(
        escape("<a href=\"x\">&"),
        "&lt;a href=&quot;x&quot;&gt;&amp;"
    );
    assert_eq!(escape("' onclick='x"), "&#39; onclick=&#39;x");
    assert!(page_html("portal.page.not_paired", &[]).starts_with("<!doctype html>"));
}
