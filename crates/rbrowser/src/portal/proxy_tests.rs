use rb_cdp::request::parse_head;
use tokio::net::TcpListener;

use super::*;

const PORTAL: &str = "http://192.168.1.20:4000";

/// The map of a single upstream.
fn map(origin: &str) -> OriginMap {
    OriginMap::new(origin, PORTAL)
}

/// The device stays paired and the portal runs.
pub(super) fn never() -> std::future::Pending<()> {
    std::future::pending()
}

#[test]
fn an_upgrade_needs_both_headers() {
    let upgrade = parse_head(
        b"GET / HTTP/1.1\r\nUpgrade: websocket\r\nConnection: keep-alive, Upgrade\r\n\r\n",
    )
    .expect("parses");
    assert!(is_upgrade(&upgrade));
    let plain = parse_head(b"GET / HTTP/1.1\r\nUpgrade: websocket\r\n\r\n").expect("parses");
    assert!(!is_upgrade(&plain));
}

fn html_head(extra: &str) -> ResponseHead {
    parse_response_head(
        format!("HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n{extra}\r\n").as_bytes(),
    )
    .expect("parses")
}

#[tokio::test]
async fn a_body_is_read_by_length_by_chunks_or_to_the_end() {
    let mut body = b"ab".to_vec();
    let mut rest: &[u8] = b"cdef";
    let read = read_body(&mut rest, &mut body, &html_head("Content-Length: 4\r\n")).await;
    assert_eq!(read.expect("reads"), WholeBody::Whole(b"abcd".to_vec()));

    let mut body = Vec::new();
    let mut rest: &[u8] = b"3\r\nabc\r\n0\r\n\r\n";
    let read = read_body(
        &mut rest,
        &mut body,
        &html_head("Transfer-Encoding: chunked\r\n"),
    )
    .await;
    assert_eq!(read.expect("reads"), WholeBody::Whole(b"abc".to_vec()));

    let mut body = Vec::new();
    let mut rest: &[u8] = b"until the end";
    let read = read_body(&mut rest, &mut body, &html_head("")).await;
    assert_eq!(
        read.expect("reads"),
        WholeBody::Whole(b"until the end".to_vec())
    );

    let mut body = Vec::new();
    let mut rest: &[u8] = b"short";
    let read = read_body(&mut rest, &mut body, &html_head("Content-Length: 99\r\n")).await;
    assert_eq!(
        read.expect("reads"),
        WholeBody::Cut,
        "the upstream closed early"
    );

    // RB-090-72 review: a malformed chunk is no page either; only a page too large is.
    let mut body = Vec::new();
    let mut rest: &[u8] = b"zz\r\nabc\r\n0\r\n\r\n";
    let read = read_body(
        &mut rest,
        &mut body,
        &html_head("Transfer-Encoding: chunked\r\n"),
    )
    .await;
    assert_eq!(read.expect("reads"), WholeBody::Cut, "malformed");

    let large = vec![b'a'; MAX_HTML + 20_000];
    let mut body = Vec::new();
    let mut rest: &[u8] = &large;
    let length = format!("Content-Length: {}\r\n", large.len());
    let read = read_body(&mut rest, &mut body, &html_head(&length)).await;
    assert_eq!(read.expect("reads"), WholeBody::TooLarge);
    assert!(body.len() > MAX_HTML, "what came stays for the stream");
}

/// A connection pair: what the test writes on the first arrives on the second.
pub(super) async fn pair() -> (TcpStream, TcpStream) {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("binds");
    let address = listener.local_addr().expect("address");
    let (client, accepted) = tokio::join!(TcpStream::connect(address), listener.accept());
    (client.expect("connects"), accepted.expect("accepts").0)
}

#[tokio::test]
async fn a_websocket_upgrade_passes_through_both_ways() {
    // The upstream: switches protocols, then echoes.
    let upstream = TcpListener::bind("127.0.0.1:0").await.expect("binds");
    let origin = format!("http://{}", upstream.local_addr().expect("address"));
    tokio::spawn(async move {
        let (mut stream, _) = upstream.accept().await.expect("accepts");
        let mut head = Vec::new();
        let mut byte = [0u8; 1];
        while head_end(&head).is_none() && stream.read(&mut byte).await.unwrap_or(0) == 1 {
            head.push(byte[0]);
        }
        assert!(String::from_utf8_lossy(&head).contains("Upgrade: websocket\r\n"));
        stream
            .write_all(b"HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\r\n")
            .await
            .expect("writes");
        let mut echo = [0u8; 4];
        stream.read_exact(&mut echo).await.expect("reads");
        stream.write_all(&echo).await.expect("echoes");
    });
    let (mut device, portal_side) = pair().await;
    let request = parse_head(
        b"GET /hmr HTTP/1.1\r\nHost: x\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\r\n",
    )
    .expect("parses");
    let upstream = Upstream::parse(&origin).expect("http");
    let forwarding = tokio::spawn(async move {
        forward(
            portal_side,
            request,
            Vec::new(),
            &upstream,
            &map(&origin),
            never(),
        )
        .await
    });
    let mut head = Vec::new();
    let mut byte = [0u8; 1];
    while head_end(&head).is_none() && device.read(&mut byte).await.unwrap_or(0) == 1 {
        head.push(byte[0]);
    }
    assert!(String::from_utf8_lossy(&head).starts_with("HTTP/1.1 101 "));
    device.write_all(b"ping").await.expect("writes");
    let mut echo = [0u8; 4];
    device
        .read_exact(&mut echo)
        .await
        .expect("the echo comes back");
    assert_eq!(&echo, b"ping");
    drop(device);
    let _ = tokio::time::timeout(Duration::from_secs(5), forwarding).await;
}

#[tokio::test]
async fn an_unreachable_upstream_is_a_bad_gateway() {
    let unused = TcpListener::bind("127.0.0.1:0").await.expect("binds");
    let origin = format!("http://{}", unused.local_addr().expect("address"));
    drop(unused);
    let (mut device, portal_side) = pair().await;
    let request = parse_head(b"GET / HTTP/1.1\r\nHost: x\r\n\r\n").expect("parses");
    let upstream = Upstream::parse(&origin).expect("http");
    assert!(
        forward(
            portal_side,
            request,
            Vec::new(),
            &upstream,
            &map(&origin),
            never()
        )
        .await
        .is_err()
    );
    let mut answer = String::new();
    device.read_to_string(&mut answer).await.expect("reads");
    assert!(answer.starts_with("HTTP/1.1 502 "), "{answer}");
}

/// An upstream that switches protocols on its first connection, then echoes until the
/// connection closes; `closed` fires when it saw the end.
async fn echo_upstream() -> (String, tokio::sync::oneshot::Receiver<()>) {
    let upstream = TcpListener::bind("127.0.0.1:0").await.expect("binds");
    let origin = format!("http://{}", upstream.local_addr().expect("address"));
    let (closed, seen) = tokio::sync::oneshot::channel();
    tokio::spawn(async move {
        let (mut stream, _) = upstream.accept().await.expect("accepts");
        let mut head = Vec::new();
        let mut byte = [0u8; 1];
        while head_end(&head).is_none() && stream.read(&mut byte).await.unwrap_or(0) == 1 {
            head.push(byte[0]);
        }
        let _ = stream
            .write_all(b"HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\r\n")
            .await;
        let mut buffer = [0u8; 64];
        while let Ok(read) = stream.read(&mut buffer).await {
            if read == 0 || stream.write_all(&buffer[..read]).await.is_err() {
                break;
            }
        }
        let _ = closed.send(());
    });
    (origin, seen)
}

#[tokio::test]
async fn a_passed_through_upgrade_ends_when_the_device_is_revoked() {
    let (origin, upstream_closed) = echo_upstream().await;
    let (mut device, portal_side) = pair().await;
    let request = parse_head(
        b"GET /hmr HTTP/1.1\r\nHost: x\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\r\n",
    )
    .expect("parses");
    let upstream = Upstream::parse(&origin).expect("http");
    let (revoke, revoked) = tokio::sync::oneshot::channel::<()>();
    let ended = async move {
        let _ = revoked.await;
    };
    let forwarding = tokio::spawn(async move {
        forward(
            portal_side,
            request,
            Vec::new(),
            &upstream,
            &map(&origin),
            ended,
        )
        .await
    });
    let mut head = Vec::new();
    let mut byte = [0u8; 1];
    while head_end(&head).is_none() && device.read(&mut byte).await.unwrap_or(0) == 1 {
        head.push(byte[0]);
    }
    assert!(String::from_utf8_lossy(&head).starts_with("HTTP/1.1 101 "));
    device.write_all(b"ping").await.expect("writes");
    let mut echo = [0u8; 4];
    device
        .read_exact(&mut echo)
        .await
        .expect("the echo comes back");
    revoke.send(()).expect("forwarding");
    let mut rest = Vec::new();
    let read = tokio::time::timeout(Duration::from_secs(5), device.read_to_end(&mut rest)).await;
    assert!(
        matches!(read, Ok(Ok(0)) | Ok(Err(_))),
        "the device's end closes: {read:?}"
    );
    let done = tokio::time::timeout(Duration::from_secs(5), forwarding).await;
    assert!(matches!(done, Ok(Ok(Ok(())))), "{done:?}");
    assert!(
        tokio::time::timeout(Duration::from_secs(5), upstream_closed)
            .await
            .is_ok(),
        "the upstream's end closes too"
    );
}

#[tokio::test]
async fn an_html_body_that_never_ends_times_out() {
    let (mut upstream, mut stalled) = tokio::io::duplex(64);
    upstream.write_all(b"<html>").await.expect("writes");
    let mut body = Vec::new();
    let read = read_body_within(
        &mut stalled,
        &mut body,
        &html_head("Content-Length: 100\r\n"),
        Duration::from_millis(50),
    )
    .await;
    assert_eq!(
        read.expect_err("timed out").kind(),
        std::io::ErrorKind::TimedOut
    );
    drop(upstream);
}

/// An upstream that answers its first request with `reply(own origin)`, then closes.
pub(super) async fn fixed_upstream(reply: impl FnOnce(&str) -> String) -> String {
    let upstream = TcpListener::bind("127.0.0.1:0").await.expect("binds");
    let origin = format!("http://{}", upstream.local_addr().expect("address"));
    let reply = reply(&origin);
    tokio::spawn(async move {
        let (mut stream, _) = upstream.accept().await.expect("accepts");
        let mut head = Vec::new();
        let mut byte = [0u8; 1];
        while head_end(&head).is_none() && stream.read(&mut byte).await.unwrap_or(0) == 1 {
            head.push(byte[0]);
        }
        let _ = stream.write_all(reply.as_bytes()).await;
        let _ = stream.shutdown().await;
    });
    origin
}

/// A script that names its own upstream, forwarded through `map(origin)`; the device's answer.
async fn script_through(map: impl FnOnce(&str) -> OriginMap) -> (String, String) {
    body_through(
        "text/javascript",
        true,
        |origin| format!("import \"{origin}/x.js\";"),
        map,
    )
    .await
}

/// A `mime` answer whose body `body(own origin)` gives, `chunked` or with a `Content-Length`,
/// forwarded through `map(origin)`; the device's answer.
pub(super) async fn body_through(
    mime: &str,
    chunked: bool,
    body: impl FnOnce(&str) -> String,
    map: impl FnOnce(&str) -> OriginMap,
) -> (String, String) {
    let origin = fixed_upstream(|origin| {
        let body = body(origin);
        let length = body.len();
        if chunked {
            format!(
                "HTTP/1.1 200 OK\r\nContent-Type: {mime}\r\nTransfer-Encoding: chunked\r\n\
                 ETag: \"v1\"\r\n\r\n{length:x}\r\n{body}\r\n0\r\n\r\n"
            )
        } else {
            format!(
                "HTTP/1.1 200 OK\r\nContent-Type: {mime}\r\nContent-Length: {length}\r\n\
                 ETag: \"v1\"\r\n\r\n{body}"
            )
        }
    })
    .await;
    let (mut device, portal_side) = pair().await;
    let request = parse_head(b"GET /app.js HTTP/1.1\r\nHost: x\r\n\r\n").expect("parses");
    let upstream = Upstream::parse(&origin).expect("http");
    let map = map(&origin);
    let forwarding = tokio::spawn(async move {
        forward(portal_side, request, Vec::new(), &upstream, &map, never()).await
    });
    let mut answer = String::new();
    let _ = tokio::time::timeout(Duration::from_secs(5), device.read_to_string(&mut answer)).await;
    let _ = tokio::time::timeout(Duration::from_secs(5), forwarding).await;
    (origin, answer)
}

/// The device's body of `answer`, its chunks decoded.
pub(super) fn chunked_body(answer: &str) -> String {
    let (_, body) = answer.split_once("\r\n\r\n").expect("a head");
    let decoded = crate::portal::inject::decode_chunked(body.as_bytes()).expect("whole chunks");
    String::from_utf8(decoded).expect("utf-8")
}

/// The head of a body the portal rewrote as it streamed: chunked anew, never cached.
fn assert_streamed_rewritten(answer: &str) {
    assert!(
        answer.contains("Transfer-Encoding: chunked\r\n"),
        "{answer}"
    );
    assert!(answer.contains("Cache-Control: no-store\r\n"), "{answer}");
    assert!(!answer.contains("ETag"), "{answer}");
    assert!(!answer.contains("Content-Length"), "{answer}");
}

/// RB-090-72: a script names the page's own upstream - without asset origins too - and reaches
/// the device rewritten as it streams.
#[tokio::test]
async fn a_script_naming_its_own_origin_is_rewritten_without_asset_origins() {
    let (_, answer) = script_through(map).await;
    assert_streamed_rewritten(&answer);
    assert_eq!(chunked_body(&answer), format!("import \"{PORTAL}/x.js\";"));
}

/// With asset origins a script is rewritten the same way.
#[tokio::test]
async fn a_script_is_rewritten_when_there_are_asset_origins() {
    let (_, answer) = script_through(|origin| {
        OriginMap::new(origin, PORTAL).with("http://localhost:5173", "http://192.168.1.20:4100")
    })
    .await;
    assert_streamed_rewritten(&answer);
    assert_eq!(chunked_body(&answer), format!("import \"{PORTAL}/x.js\";"));
}

/// RB-090-69, RB-090-72: an API answer names its images with escaped slashes; with a
/// `Content-Length` and no asset origins its upstream URLs point at the portal, still escaped.
#[tokio::test]
async fn an_escaped_json_body_is_rewritten() {
    let (_, answer) = body_through(
        "application/json",
        false,
        |origin| format!("{{\"img\":\"{}\\/a.png\"}}", origin.replace('/', "\\/")),
        map,
    )
    .await;
    assert_streamed_rewritten(&answer);
    assert_eq!(
        chunked_body(&answer),
        r#"{"img":"http:\/\/192.168.1.20:4000\/a.png"}"#
    );
}

/// RB-090-69: the "unmapped loopback origin" evidence comes from every body the portal rewrites
/// - a style sheet, a script or JSON from any listener, not only the page's HTML, read whole or
/// streamed (RB-090-72) - plain or escaped, the origin only.
#[tokio::test]
async fn an_unmapped_loopback_origin_in_json_is_logged_without_its_path() {
    let log = super::super::log_tests::Log::default();
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_ansi(false)
        .with_writer(log.clone())
        .finish();
    // A current-thread runtime: the forwarding task runs on this thread, under this default.
    let _default = tracing::subscriber::set_default(subscriber);
    let (_, answer) = body_through(
        "application/json",
        false,
        |_| r#"{"img":"http:\/\/127.0.0.1:9471\/a.png?token=secret"}"#.to_owned(),
        |origin| {
            OriginMap::new(origin, PORTAL).with("http://localhost:5173", "http://192.168.1.20:4100")
        },
    )
    .await;
    assert!(answer.contains("127.0.0.1:9471"), "{answer}");
    let text = log.text();
    assert!(text.contains("origin=http://127.0.0.1:9471"), "{text}");
    assert!(
        !text.contains("secret") && !text.contains("a.png"),
        "{text}"
    );
}

/// RB-090-72: a chunked JSON answer (a stream, a long poll) is rewritten as it streams - never
/// read whole first.
#[tokio::test]
async fn a_chunked_json_body_is_rewritten_as_it_streams() {
    let (_, answer) = body_through(
        "application/json",
        true,
        |origin| format!("{{\"img\":\"{origin}/media/a.webp\"}}"),
        map,
    )
    .await;
    assert_streamed_rewritten(&answer);
    assert_eq!(
        chunked_body(&answer),
        format!("{{\"img\":\"{PORTAL}/media/a.webp\"}}")
    );
}
