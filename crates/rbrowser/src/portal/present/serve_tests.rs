//! The presentation's listener end to end over real sockets on 127.0.0.1, with requests as
//! cloudflared forwards them (`Host` the tunnel's name): the join page, the one-time link, the
//! viewer limit, the path rules before the dev server, the headers, the viewer's socket.

use std::net::Ipv4Addr;
use std::sync::atomic::{AtomicUsize, Ordering};

use futures_util::{SinkExt as _, StreamExt as _};
use tokio::io::AsyncWriteExt as _;
use tokio::net::TcpListener;
use tokio::runtime::Handle;
use tokio::sync::mpsc;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::HeaderValue;

use super::*;
use crate::portal::present::paths::PathRules;
use crate::portal::server::Options;

const HOST: &str = "quick-brown-fox.trycloudflare.com";
const ORIGIN: &str = "https://quick-brown-fox.trycloudflare.com";
const AGENT: &str = "/*portal agent*/";

fn start(max_viewers: usize, ttl: Duration, address: bool) -> Arc<PortalServer> {
    let settings = rb_config::PresentationSettings::default();
    let public = Arc::new(Public::new(
        PathRules::new("/", &settings.allow_paths, &settings.deny_paths),
        true,
    ));
    if address {
        assert!(public.set_address(ORIGIN));
    }
    PortalServer::start_public(
        &Handle::current(),
        Options {
            ip: Ipv4Addr::UNSPECIFIED,
            port: None,
            pairing_ttl: ttl,
            max_devices: max_viewers,
            agent: Arc::from(AGENT),
            on_change: Arc::new(|| {}),
        },
        public,
    )
    .expect("listens")
}

fn live(max_viewers: usize) -> Arc<PortalServer> {
    start(max_viewers, Duration::from_secs(600), true)
}

async fn raw(server: &PortalServer, request: &str) -> String {
    let mut stream = TcpStream::connect(server.address())
        .await
        .expect("connects");
    stream.write_all(request.as_bytes()).await.expect("writes");
    let mut answer = Vec::new();
    let _ = tokio::time::timeout(Duration::from_secs(5), stream.read_to_end(&mut answer)).await;
    String::from_utf8_lossy(&answer).into_owned()
}

async fn get(server: &PortalServer, path: &str, extra: &str) -> String {
    raw(
        server,
        &format!("GET {path} HTTP/1.1\r\nHost: {HOST}\r\nUser-Agent: Customer\r\n{extra}\r\n"),
    )
    .await
}

async fn post_token(server: &PortalServer, token: &str, extra: &str) -> String {
    raw(
        server,
        &format!(
            "POST {PAIR_PATH} HTTP/1.1\r\nHost: {HOST}\r\nOrigin: {ORIGIN}\r\n\
             Content-Type: text/plain\r\nContent-Length: {}\r\n{extra}\r\n{token}",
            token.len()
        ),
    )
    .await
}

fn status(answer: &str) -> u16 {
    answer
        .split(' ')
        .nth(1)
        .and_then(|code| code.parse().ok())
        .unwrap_or(0)
}

fn token_of(link: &ViewerLinkUrl) -> String {
    link.0
        .split_once("#t=")
        .map(|(_, token)| token.to_owned())
        .expect("a token in the fragment")
}

struct ViewerLinkUrl(String);

fn issue(server: &PortalServer) -> ViewerLinkUrl {
    let link = server.issue_link().expect("random").expect("an address");
    assert!(link.url.starts_with(&format!("{ORIGIN}{JOIN_PATH}#t=")));
    ViewerLinkUrl(link.url)
}

/// Joins with a fresh link; the viewer's cookie header line.
async fn join(server: &PortalServer) -> String {
    let token = token_of(&issue(server));
    let answer = post_token(server, &token, "").await;
    assert_eq!(status(&answer), 200, "{answer}");
    let cookie = answer
        .lines()
        .find_map(|line| line.strip_prefix("Set-Cookie: "))
        .expect("a cookie");
    assert!(
        cookie.ends_with("; Path=/; HttpOnly; Secure; SameSite=Strict"),
        "{cookie}"
    );
    assert!(answer.ends_with(&format!("{{\"next\":\"{FOLLOW_PATH}\"}}")));
    format!("Cookie: {}\r\n", cookie.split(';').next().expect("a pair"))
}

/// An upstream that counts its requests and hands them out.
async fn upstream(answer: &'static str) -> (String, Arc<AtomicUsize>, mpsc::Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("binds");
    let origin = format!("http://{}", listener.local_addr().expect("address"));
    let count = Arc::new(AtomicUsize::new(0));
    let counted = count.clone();
    let (seen, requests) = mpsc::channel(8);
    tokio::spawn(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            counted.fetch_add(1, Ordering::SeqCst);
            let mut head = Vec::new();
            let mut byte = [0u8; 1];
            while rb_cdp::request::head_end(&head).is_none()
                && stream.read(&mut byte).await.unwrap_or(0) == 1
            {
                head.push(byte[0]);
            }
            let _ = seen.send(String::from_utf8_lossy(&head).into_owned()).await;
            let _ = stream.write_all(answer.as_bytes()).await;
            let _ = stream.shutdown().await;
        }
    });
    (origin, count, requests)
}

#[tokio::test]
async fn before_the_tunnel_named_its_address_everything_answers_503() {
    let server = start(3, Duration::from_secs(600), false);
    assert_eq!(status(&get(&server, JOIN_PATH, "").await), 503);
    assert_eq!(status(&get(&server, "/", "").await), 503);
    assert!(
        server.issue_link().expect("random").is_none(),
        "no link without an address"
    );
}

#[tokio::test]
async fn another_host_answers_403() {
    let server = live(3);
    let other = raw(
        &server,
        &format!(
            "GET {JOIN_PATH} HTTP/1.1\r\nHost: 127.0.0.1:{}\r\n\r\n",
            server.address().port()
        ),
    )
    .await;
    assert_eq!(status(&other), 403);
}

#[tokio::test]
async fn the_join_page_posts_the_fragment_and_drops_it() {
    let server = live(3);
    let answer = get(&server, JOIN_PATH, "").await;
    assert_eq!(status(&answer), 200, "{answer}");
    assert!(answer.contains("location.hash"), "{answer}");
    assert!(answer.contains("history.replaceState"), "{answer}");
    assert!(
        answer.contains(&format!("fetch(\"{PAIR_PATH}\"")),
        "{answer}"
    );
    assert!(answer.contains("Cache-Control: no-store"));
    assert!(answer.contains("<meta name=\"referrer\" content=\"no-referrer\">"));
}

/// Acceptance: the link works exactly once; a spent or expired one shows a page without
/// anything of the project.
#[tokio::test]
async fn a_link_works_exactly_once() {
    let server = live(3);
    let token = token_of(&issue(&server));
    let first = post_token(&server, &token, "User-Agent: Customer\r\n").await;
    assert_eq!(status(&first), 200, "{first}");
    let second = post_token(&server, &token, "").await;
    assert_eq!(status(&second), 403, "{second}");
    assert!(
        second.contains(&rb_i18n::t("portal.present.page.invalid")),
        "{second}"
    );
    assert!(!second.contains("Set-Cookie"));
    assert_eq!(server.devices().len(), 1);
    assert_eq!(server.devices()[0].info.name, "Customer");
}

#[tokio::test]
async fn an_expired_link_is_refused() {
    let server = start(3, Duration::from_millis(1), true);
    let token = token_of(&issue(&server));
    tokio::time::sleep(Duration::from_millis(20)).await;
    let answer = post_token(&server, &token, "").await;
    assert_eq!(status(&answer), 403);
    assert!(
        answer.contains(&rb_i18n::t("portal.present.page.expired")),
        "{answer}"
    );
    assert!(server.devices().is_empty());
}

/// The token never travels in a URL: a `GET` with it in the query pairs nothing.
#[tokio::test]
async fn the_token_is_taken_from_a_post_body_only() {
    let server = live(3);
    let token = token_of(&issue(&server));
    let answer = get(&server, &format!("{PAIR_PATH}?t={token}"), "").await;
    assert_eq!(status(&answer), 405);
    assert_eq!(
        status(&post_token(&server, &token, "").await),
        200,
        "still open"
    );
}

#[tokio::test]
async fn a_post_from_another_origin_is_refused() {
    let server = live(3);
    let token = token_of(&issue(&server));
    let answer = raw(
        &server,
        &format!(
            "POST {PAIR_PATH} HTTP/1.1\r\nHost: {HOST}\r\nOrigin: https://evil.example.com\r\n\
             Content-Length: {}\r\n\r\n{token}",
            token.len()
        ),
    )
    .await;
    assert_eq!(status(&answer), 403);
    assert!(server.devices().is_empty());
}

/// Acceptance: past `maxViewers` every further viewer is refused.
#[tokio::test]
async fn past_the_viewer_limit_every_further_viewer_is_refused() {
    let server = live(1);
    let _cookie = join(&server).await;
    let token = token_of(&issue(&server));
    let answer = post_token(&server, &token, "").await;
    assert_eq!(status(&answer), 403);
    assert!(
        answer.contains(&rb_i18n::t("portal.present.page.full")),
        "{answer}"
    );
    assert_eq!(server.devices().len(), 1);
}

/// Acceptance: a path of `denyPaths` answers 404 and the dev server never sees it; without the
/// cookie nothing reaches it either.
#[tokio::test]
async fn a_denied_path_answers_404_without_reaching_the_dev_server() {
    let (origin, count, _requests) =
        upstream("HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok").await;
    let server = live(3);
    server.set_page(Some(format!("{origin}/")), vec![origin.clone()]);
    let cookie = join(&server).await;
    for path in [
        "/admin",
        "/admin/users",
        "/.env",
        "/%2Eenv",
        "/x/../admin",
        "/@fs/etc/passwd",
    ] {
        let answer = get(&server, path, &cookie).await;
        assert_eq!(status(&answer), 404, "{path}: {answer}");
        assert!(answer.contains(AGENT_TAG), "the viewer follows on from it");
    }
    assert_eq!(status(&get(&server, "/shop", "").await), 403, "no cookie");
    assert_eq!(
        count.load(Ordering::SeqCst),
        0,
        "the dev server saw nothing"
    );
}

#[tokio::test]
async fn an_allowed_path_is_forwarded_without_cookies_or_server_details() {
    let (origin, count, mut requests) = upstream(
        "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nSet-Cookie: session=presenter\r\n\
         Server: nginx/1.27\r\nX-Powered-By: PHP/8.4\r\nContent-Length: 2\r\n\r\nok",
    )
    .await;
    let server = live(3);
    server.set_page(Some(format!("{origin}/")), vec![origin.clone()]);
    let cookie = join(&server).await;
    let answer = get(
        &server,
        "/shop?x=1",
        &format!("{cookie}Cf-Connecting-Ip: 203.0.113.9\r\n"),
    )
    .await;
    assert_eq!(status(&answer), 200, "{answer}");
    assert!(answer.ends_with("ok"));
    for gone in ["Set-Cookie", "nginx", "PHP"] {
        assert!(!answer.contains(gone), "{gone}: {answer}");
    }
    let seen = requests.recv().await.expect("the dev server was asked");
    assert!(seen.starts_with("GET /shop?x=1 HTTP/1.1\r\n"), "{seen}");
    assert!(!seen.contains(COOKIE_NAME), "{seen}");
    assert!(!seen.contains("203.0.113.9"), "{seen}");
    assert_eq!(count.load(Ordering::SeqCst), 1);
}

async fn socket(server: &PortalServer, cookie: &str) -> WebSocketStream<TcpStream> {
    let stream = TcpStream::connect(server.address())
        .await
        .expect("connects");
    let mut request = format!("ws://{HOST}{SOCKET_PATH}")
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

/// Acceptance: a viewer's input never reaches the grid - its socket takes reports only, and an
/// event it sends is dropped (0 counted, nothing replayed). The viewer learns the pointer flash;
/// stopping the presentation closes its socket.
#[tokio::test]
async fn a_viewer_follows_and_drives_nothing() {
    let server = live(3);
    let allow = vec!["http://localhost:5173".to_owned()];
    server.set_page(Some("http://localhost:5173/a".to_owned()), allow);
    let cookie = join(&server).await;
    let mut ws = socket(&server, &cookie).await;
    assert_eq!(
        next_text(&mut ws).await.as_deref(),
        Some(r#"{"kind":"navigate","path":"/a"}"#)
    );
    assert_eq!(
        next_text(&mut ws).await.as_deref(),
        Some(r#"{"kind":"present","pointer":true}"#)
    );
    let click = r##"{"kind":"event","event":{"type":"click","target":{"selector":"#buy"}}}"##;
    ws.send(Message::Text(click.into())).await.expect("sends");
    ws.send(Message::Text(
        r#"{"kind":"navigate","path":"/admin"}"#.into(),
    ))
    .await
    .expect("sends");
    tokio::time::sleep(Duration::from_millis(100)).await;
    let viewer = &server.devices()[0];
    assert!(viewer.connected);
    assert_eq!(
        (
            viewer.counts.replayed,
            viewer.counts.failed,
            viewer.counts.skipped
        ),
        (0, 0, 0)
    );
    server.stop();
    assert_eq!(next_text(&mut ws).await, None, "the socket closes");
    assert!(server.devices().is_empty(), "every viewer was disconnected");
}

#[tokio::test]
async fn the_follow_page_carries_the_agent_for_a_viewer_only() {
    let server = live(3);
    assert_eq!(status(&get(&server, FOLLOW_PATH, "").await), 403);
    let cookie = join(&server).await;
    let answer = get(&server, FOLLOW_PATH, &cookie).await;
    assert_eq!(status(&answer), 200);
    assert!(answer.contains(AGENT_TAG));
    let agent = get(&server, AGENT_PATH, &cookie).await;
    assert!(agent.ends_with(AGENT));
}

#[test]
fn a_page_of_its_own_escapes_and_names_no_project() {
    let html = page_html("portal.present.page.not_shown", false);
    assert!(html.starts_with("<!doctype html>"));
    assert!(!html.contains(AGENT_TAG));
    assert!(page_html("portal.present.page.follow", true).contains(AGENT_TAG));
    assert_eq!(
        join_html().matches("</script>").count(),
        1,
        "one script, ended once"
    );
}
