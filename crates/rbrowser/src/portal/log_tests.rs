//! "Log and diagnostics carry no token or cookie" (RB-080-05): a pairing, a refused pairing, a
//! request with the cookie, a forged cookie and a disconnect, with every log line at TRACE written
//! into a buffer; the buffer is searched for the pairing token and the device session, and the
//! diagnostics' mask (`rb_diag::redact`, which the log file and the ZIP go through) hides both
//! while they are in use.

use std::io::Write;
use std::net::Ipv4Addr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio::net::TcpStream;
use tokio::runtime::Handle;
use tracing_subscriber::fmt::MakeWriter;

use super::server::{Options, PortalServer};
use crate::cdp::lock;

#[derive(Clone, Default)]
struct Log(Arc<Mutex<Vec<u8>>>);

impl Write for Log {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        lock(&self.0).extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> MakeWriter<'a> for Log {
    type Writer = Log;

    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

async fn get(server: &PortalServer, path: &str, extra: &str) -> String {
    let mut stream = TcpStream::connect(server.address())
        .await
        .expect("connects");
    let request = format!(
        "GET {path} HTTP/1.1\r\nHost: {}\r\n{extra}\r\n",
        server.address()
    );
    stream.write_all(request.as_bytes()).await.expect("writes");
    let mut answer = Vec::new();
    let _ = tokio::time::timeout(Duration::from_secs(5), stream.read_to_end(&mut answer)).await;
    String::from_utf8_lossy(&answer).into_owned()
}

/// nextest runs every test in a process of its own: the global subscriber is this test's.
#[tokio::test(flavor = "multi_thread")]
async fn no_token_or_cookie_reaches_the_log_or_the_diagnostics() {
    let log = Log::default();
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .with_ansi(false)
        .with_writer(log.clone())
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("the test's own process");

    let server = PortalServer::start(
        &Handle::current(),
        Options {
            ip: Ipv4Addr::LOCALHOST,
            port: None,
            pairing_ttl: Duration::from_secs(300),
            max_devices: 5,
            agent: Arc::from(""),
            on_change: Arc::new(|| {}),
        },
    )
    .expect("listens");
    let pairing = server.issue_pairing().expect("random");
    let path = pairing.url.trim_start_matches(server.origin()).to_owned();
    let token = path.rsplit('=').next().expect("a token").to_owned();
    let paired = get(&server, &path, "User-Agent: Phone\r\n").await;
    let _ = get(&server, &path, "").await;
    let session = paired
        .lines()
        .find_map(|line| line.strip_prefix("Set-Cookie: __rbportal="))
        .and_then(|value| value.split(';').next())
        .expect("a session")
        .to_owned();
    let cookie = format!("Cookie: __rbportal={session}\r\n");
    let _ = get(&server, "/", &cookie).await;
    let _ = get(&server, "/", "Cookie: __rbportal=forged\r\n").await;
    // While they live, the mask hides both; once given up they leave it (Charge 10 review L3).
    let masked = rb_diag::redact(&format!("{} {session} {token}", pairing.url));
    assert!(
        !masked.contains(&token) && !masked.contains(&session),
        "{masked}"
    );
    let device = server.devices()[0].info.id;
    server.disconnect(device);
    let _ = get(&server, "/", &cookie).await;
    server.stop();

    let text = String::from_utf8_lossy(&lock(&log.0)).into_owned();
    assert!(text.contains("paired"), "the run logged: {text}");
    assert!(!text.contains(&token), "the pairing token is in the log");
    assert!(!text.contains(&session), "the device session is in the log");
}
