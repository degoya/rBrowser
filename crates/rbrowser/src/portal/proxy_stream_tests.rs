//! RB-090-72: a streamed answer - a long poll, an event stream - reaches the device rewritten
//! while it still streams, not after it ended.

use rb_cdp::request::parse_head;
use tokio::net::TcpListener;

use super::tests::{chunked_body, fixed_upstream, never, pair};
use super::*;

const PORTAL: &str = "http://192.168.1.20:4000";

#[tokio::test]
async fn a_long_poll_reaches_the_device_rewritten_before_it_ends() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("binds");
    let origin = format!("http://{}", listener.local_addr().expect("address"));
    let first = format!("data: {{\"img\":\"{origin}/media/a.webp\"}}\n\n");
    let (release, released) = tokio::sync::oneshot::channel::<()>();
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("accepts");
        let mut head = Vec::new();
        let mut byte = [0u8; 1];
        while head_end(&head).is_none() && stream.read(&mut byte).await.unwrap_or(0) == 1 {
            head.push(byte[0]);
        }
        let start = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nTransfer-Encoding: chunked\r\n\
             \r\n{:x}\r\n{first}\r\n",
            first.len()
        );
        let _ = stream.write_all(start.as_bytes()).await;
        // The poll stays open until the test has seen its first message.
        let _ = released.await;
        let _ = stream.write_all(b"0\r\n\r\n").await;
        let _ = stream.shutdown().await;
    });
    let (mut device, portal_side) = pair().await;
    let request = parse_head(b"GET /poll HTTP/1.1\r\nHost: x\r\n\r\n").expect("parses");
    let upstream = Upstream::parse(&origin).expect("http");
    let map = OriginMap::new(&origin, PORTAL);
    let forwarding = tokio::spawn(async move {
        forward(portal_side, request, Vec::new(), &upstream, &map, never()).await
    });
    let expected = format!("data: {{\"img\":\"{PORTAL}/media/a.webp\"}}\n\n");
    let mut seen = Vec::new();
    let arrived = tokio::time::timeout(Duration::from_secs(5), async {
        let mut buffer = [0u8; 4096];
        loop {
            let read = device.read(&mut buffer).await.unwrap_or(0);
            if read == 0 {
                return false;
            }
            seen.extend_from_slice(&buffer[..read]);
            if String::from_utf8_lossy(&seen).contains(&expected) {
                return true;
            }
        }
    })
    .await;
    assert_eq!(arrived, Ok(true), "{}", String::from_utf8_lossy(&seen));
    let _ = release.send(());
    let mut rest = Vec::new();
    let _ = tokio::time::timeout(Duration::from_secs(5), device.read_to_end(&mut rest)).await;
    seen.extend_from_slice(&rest);
    let _ = tokio::time::timeout(Duration::from_secs(5), forwarding).await;
    assert!(
        seen.ends_with(b"0\r\n\r\n"),
        "{}",
        String::from_utf8_lossy(&seen)
    );
}

/// `reply` (with the upstream's origin) forwarded for `GET /` through the map of that upstream:
/// the device's answer.
async fn through(reply: impl FnOnce(&str) -> String) -> (String, String) {
    let origin = fixed_upstream(reply).await;
    let (mut device, portal_side) = pair().await;
    let request = parse_head(b"GET / HTTP/1.1\r\nHost: x\r\n\r\n").expect("parses");
    let upstream = Upstream::parse(&origin).expect("http");
    let map = OriginMap::new(&origin, PORTAL);
    let forwarding = tokio::spawn(async move {
        forward(portal_side, request, Vec::new(), &upstream, &map, never()).await
    });
    let mut answer = String::new();
    let _ = tokio::time::timeout(Duration::from_secs(5), device.read_to_string(&mut answer)).await;
    let _ = tokio::time::timeout(Duration::from_secs(5), forwarding).await;
    (origin, answer)
}

/// RB-090-72 review: an `HTTP/1.0` upstream (`python -m http.server`) ends its body by closing;
/// the device gets it rewritten and chunked as `HTTP/1.1`, never `HTTP/1.0` with chunks.
#[tokio::test]
async fn an_http_1_0_upstream_is_rewritten_for_the_device_as_http_1_1() {
    let (_, answer) = through(|origin| {
        format!("HTTP/1.0 200 OK\r\nContent-Type: text/javascript\r\n\r\nimport \"{origin}/x.js\";")
    })
    .await;
    assert!(answer.starts_with("HTTP/1.1 200 OK\r\n"), "{answer}");
    assert!(
        answer.contains("Transfer-Encoding: chunked\r\n"),
        "{answer}"
    );
    assert_eq!(chunked_body(&answer), format!("import \"{PORTAL}/x.js\";"));
}

/// RB-090-72 review: the page's own HTML that ends before its length (or with a broken chunk)
/// is a bad gateway - not a cut page streamed on without the agent.
#[tokio::test]
async fn a_page_cut_short_is_a_bad_gateway() {
    let (_, answer) = through(|_| {
        "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: 100\r\n\r\n<html>short"
            .to_owned()
    })
    .await;
    assert!(answer.starts_with("HTTP/1.1 502 "), "{answer}");
    assert!(!answer.contains("<html>"), "{answer}");
}

/// RB-090-75: the page's own HTML, read whole for the agent, names its images with `:` and `/`
/// as character references (an attribute escaper's `src` and `srcset`); they point at the portal
/// in the same form, and an encoded loopback origin the map does not forward is logged as its
/// plain origin, without its path.
#[tokio::test]
async fn a_page_with_entity_encoded_origins_is_rewritten_whole_and_its_evidence_logged() {
    let log = super::super::log_tests::Log::default();
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_ansi(false)
        .with_writer(log.clone())
        .finish();
    // A current-thread runtime: the forwarding task runs on this thread, under this default.
    let _default = tracing::subscriber::set_default(subscriber);
    let encode = |origin: &str| origin.replace(':', "&#x3A;").replace('/', "&#x2F;");
    let (_, answer) = through(|origin| {
        let own = encode(origin);
        let page = format!(
            "<html><body><img src=\"{own}&#x2F;media&#x2F;a.webp\" \
             srcset=\"{own}&#x2F;media&#x2F;a-1152.webp&#x20;1152w\">\
             <img src=\"http&#x3A;&#x2F;&#x2F;127.0.0.1&#x3A;9481&#x2F;b.png&#x3F;token&#x3D;secret\">\
             </body></html>"
        );
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\r\n{page}",
            page.len()
        )
    })
    .await;
    assert!(answer.starts_with("HTTP/1.1 200 "), "{answer}");
    assert!(
        !answer.contains("Transfer-Encoding"),
        "read whole: {answer}"
    );
    let portal = encode(PORTAL);
    assert!(
        answer.contains(&format!("src=\"{portal}&#x2F;media&#x2F;a.webp\"")),
        "{answer}"
    );
    assert!(
        answer.contains(&format!("srcset=\"{portal}&#x2F;media&#x2F;a-1152.webp")),
        "{answer}"
    );
    let text = log.text();
    assert!(text.contains("origin=http://127.0.0.1:9481"), "{text}");
    assert!(
        !text.contains("secret") && !text.contains("b.png"),
        "{text}"
    );
}
