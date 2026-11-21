//! "Token and link are in no log, no MCP answer and no diagnostics ZIP" (RB-096-05): a link
//! issued, joined, refused, used with the cookie and with a forged one, wrong tokens until the
//! link is spent, and the stop, with every log line at TRACE written into a buffer; the buffer
//! holds neither the token, the link nor the viewer's session, and the diagnostics' mask
//! (`rb_diag::redact`, which the log file and the ZIP go through) hides them.

use std::net::Ipv4Addr;
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio::net::TcpStream;
use tokio::runtime::Handle;

use super::paths::PathRules;
use super::public::Public;
use crate::portal::log_tests::Log;
use crate::portal::server::{Options, PortalServer};

const HOST: &str = "quick-brown-fox.trycloudflare.com";

async fn send(server: &PortalServer, request: String) -> String {
    let mut stream = TcpStream::connect(server.address())
        .await
        .expect("connects");
    stream.write_all(request.as_bytes()).await.expect("writes");
    let mut answer = Vec::new();
    let _ = tokio::time::timeout(Duration::from_secs(5), stream.read_to_end(&mut answer)).await;
    String::from_utf8_lossy(&answer).into_owned()
}

async fn post(server: &PortalServer, token: &str) -> String {
    send(
        server,
        format!(
            "POST /__rbrowser/pair HTTP/1.1\r\nHost: {HOST}\r\nContent-Length: {}\r\n\r\n{token}",
            token.len()
        ),
    )
    .await
}

async fn get(server: &PortalServer, path: &str, extra: &str) -> String {
    send(
        server,
        format!("GET {path} HTTP/1.1\r\nHost: {HOST}\r\n{extra}\r\n"),
    )
    .await
}

/// nextest runs every test in a process of its own: the global subscriber is this test's.
#[tokio::test(flavor = "multi_thread")]
async fn no_token_link_or_session_reaches_the_log_or_the_diagnostics() {
    let log = Log::default();
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .with_ansi(false)
        .with_writer(log.clone())
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("the test's own process");

    let public = Arc::new(Public::new(
        PathRules::new("/", &[], &["/admin".to_owned()]),
        true,
    ));
    assert!(public.set_address(&format!("https://{HOST}")));
    let server = PortalServer::start_public(
        &Handle::current(),
        Options {
            ip: Ipv4Addr::LOCALHOST,
            port: None,
            pairing_ttl: Duration::from_secs(600),
            max_devices: 3,
            agent: Arc::from(""),
            on_change: Arc::new(|| {}),
        },
        public,
    )
    .expect("listens");
    let link = server
        .issue_link()
        .expect("random")
        .expect("an address")
        .url;
    let token = link.split_once("#t=").expect("a fragment").1.to_owned();
    let joined = post(&server, &token).await;
    let _ = post(&server, &token).await;
    let session = joined
        .lines()
        .find_map(|line| line.strip_prefix("Set-Cookie: __rbportal="))
        .and_then(|value| value.split(';').next())
        .expect("a session")
        .to_owned();
    let cookie = format!("Cookie: __rbportal={session}\r\n");
    let _ = get(&server, "/__rbrowser/follow", &cookie).await;
    let _ = get(&server, "/admin", &cookie).await;
    let _ = get(&server, "/", "Cookie: __rbportal=forged\r\n").await;
    let second = server
        .issue_link()
        .expect("random")
        .expect("an address")
        .url;
    let second_token = second.split_once("#t=").expect("a fragment").1.to_owned();
    for _ in 0..super::link::MAX_LINK_FAILURES {
        let _ = post(&server, "00000000000000000000000000000000").await;
    }
    // While they live, the mask hides them all.
    let masked = rb_diag::redact(&format!("{link} {second} {session}"));
    for secret in [&token, &second_token, &session] {
        assert!(!masked.contains(secret.as_str()), "{masked}");
    }
    server.stop();

    let text = log.text();
    assert!(text.contains("joined"), "the run logged: {text}");
    for secret in [&token, &second_token, &session, &link, &second] {
        assert!(!text.contains(secret.as_str()), "a secret is in the log");
    }
}
