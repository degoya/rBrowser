use rb_cdp::request::parse_head;
use tokio::net::TcpListener;

use super::*;

const PORTAL: &str = "http://192.168.1.20:4000";

/// The device stays paired and the portal runs.
fn never() -> std::future::Pending<()> {
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
    assert_eq!(read.expect("reads").as_deref(), Some(&b"abcd"[..]));

    let mut body = Vec::new();
    let mut rest: &[u8] = b"3\r\nabc\r\n0\r\n\r\n";
    let read = read_body(
        &mut rest,
        &mut body,
        &html_head("Transfer-Encoding: chunked\r\n"),
    )
    .await;
    assert_eq!(read.expect("reads").as_deref(), Some(&b"abc"[..]));

    let mut body = Vec::new();
    let mut rest: &[u8] = b"until the end";
    let read = read_body(&mut rest, &mut body, &html_head("")).await;
    assert_eq!(read.expect("reads").as_deref(), Some(&b"until the end"[..]));

    let mut body = Vec::new();
    let mut rest: &[u8] = b"short";
    let read = read_body(&mut rest, &mut body, &html_head("Content-Length: 99\r\n")).await;
    assert_eq!(read.expect("reads"), None, "a cut body gets no agent");
}

/// A connection pair: what the test writes on the first arrives on the second.
async fn pair() -> (TcpStream, TcpStream) {
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
        forward(portal_side, request, Vec::new(), &upstream, PORTAL, never()).await
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
        forward(portal_side, request, Vec::new(), &upstream, PORTAL, never())
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
        forward(portal_side, request, Vec::new(), &upstream, PORTAL, ended).await
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
