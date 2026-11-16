use rb_cdp::request::parse_head;

use super::*;
use crate::portal::origins::Upstream;

const PORTAL: &str = "http://192.168.1.20:4000";

fn upstream() -> Upstream {
    Upstream::parse("http://localhost:5173").expect("parses")
}

fn lines(text: &str) -> Vec<&str> {
    text.split("\r\n").collect()
}

#[test]
fn the_request_names_the_upstream_and_loses_the_portal_cookie() {
    let head = parse_head(
        b"GET /app?x=1 HTTP/1.1\r\nHost: 192.168.1.20:4000\r\nOrigin: http://192.168.1.20:4000\r\n\
          Referer: http://192.168.1.20:4000/start\r\nCookie: a=1; __rbportal=secret; b=2\r\n\
          Accept-Encoding: gzip, br\r\nConnection: keep-alive\r\nProxy-Authorization: x\r\n\r\n",
    )
    .expect("parses");
    let upstream = upstream();
    let text = request_head(&head, &upstream, PORTAL, false);
    let lines = lines(&text);
    assert_eq!(lines[0], "GET /app?x=1 HTTP/1.1");
    assert!(lines.contains(&"Host: localhost:5173"));
    assert!(lines.contains(&"Origin: http://localhost:5173"));
    assert!(lines.contains(&"Referer: http://localhost:5173/start"));
    assert!(lines.contains(&"Cookie: a=1; b=2"));
    assert!(lines.contains(&"Accept-Encoding: identity"));
    assert!(lines.contains(&"Connection: close"));
    assert!(!text.contains("secret"), "{text}");
    assert!(!text.contains("Proxy-Authorization"));
    assert!(!text.contains("keep-alive"));
    assert!(text.ends_with("\r\n\r\n"));
}

#[test]
fn a_request_with_only_the_portal_cookie_sends_none() {
    let head = parse_head(b"GET / HTTP/1.1\r\nHost: h\r\nCookie: __rbportal=secret\r\n\r\n")
        .expect("parses");
    let text = request_head(&head, &upstream(), PORTAL, false);
    assert!(!text.contains("Cookie"), "{text}");
}

#[test]
fn an_upgrade_keeps_its_headers() {
    let head = parse_head(
        b"GET /?token=x HTTP/1.1\r\nHost: h\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\
          Sec-WebSocket-Key: k\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Protocol: vite-hmr\r\n\r\n",
    )
    .expect("parses");
    let text = request_head(&head, &upstream(), PORTAL, true);
    let lines = lines(&text);
    assert!(lines.contains(&"Upgrade: websocket"));
    assert!(lines.contains(&"Connection: Upgrade"));
    assert!(lines.contains(&"Sec-WebSocket-Protocol: vite-hmr"));
    assert!(!lines.contains(&"Connection: close"));
}

#[test]
fn a_response_head_parses() {
    let head = parse_response_head(
        b"HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: 12\r\n\r\nbody",
    )
    .expect("parses");
    assert_eq!(head.status, 200);
    assert!(head.is_html());
    assert_eq!(head.content_length(), Some(12));
    assert!(!head.is_chunked());
    assert!(parse_response_head(b"garbage\r\n\r\n").is_err());
}

#[test]
fn the_response_moves_locations_and_cookies_to_the_portal() {
    let head = parse_response_head(
        b"HTTP/1.1 302 Found\r\nLocation: http://localhost:5173/login\r\n\
          Set-Cookie: sid=1; Domain=localhost; Path=/; HttpOnly\r\nSet-Cookie: __rbportal=evil\r\n\
          Strict-Transport-Security: max-age=1\r\nConnection: keep-alive\r\n\
          Transfer-Encoding: chunked\r\n\r\n",
    )
    .expect("parses");
    let text = response_head(&head, &upstream(), PORTAL, None);
    let lines = lines(&text);
    assert_eq!(lines[0], "HTTP/1.1 302 Found");
    assert!(lines.contains(&"Location: http://192.168.1.20:4000/login"));
    assert!(lines.contains(&"Set-Cookie: sid=1; Path=/; HttpOnly"));
    assert!(
        lines.contains(&"Transfer-Encoding: chunked"),
        "streamed as it came"
    );
    assert!(lines.contains(&"Connection: close"));
    assert!(!text.contains("__rbportal"));
    assert!(!text.contains("Strict-Transport-Security"));
}

#[test]
fn a_rewritten_body_gets_its_new_length() {
    let head = parse_response_head(
        b"HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nTransfer-Encoding: chunked\r\n\r\n",
    )
    .expect("parses");
    let text = response_head(&head, &upstream(), PORTAL, Some(42));
    let lines = lines(&text);
    assert!(lines.contains(&"Content-Length: 42"));
    assert!(!text.contains("Transfer-Encoding"));
}

#[test]
fn a_switching_protocols_answer_keeps_the_upgrade() {
    let head = parse_response_head(
        b"HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\
          Sec-WebSocket-Accept: a\r\n\r\n",
    )
    .expect("parses");
    let text = response_head(&head, &upstream(), PORTAL, None);
    assert!(text.contains("Upgrade: websocket\r\n"));
    assert!(text.contains("Connection: Upgrade\r\n"));
    assert!(!text.contains("Connection: close"));
}

#[test]
fn cors_names_the_portal_origin_without_a_path() {
    let head = parse_response_head(
        b"HTTP/1.1 200 OK\r\nAccess-Control-Allow-Origin: http://localhost:5173\r\n\r\n",
    )
    .expect("parses");
    let text = response_head(&head, &upstream(), PORTAL, None);
    assert!(
        text.contains("Access-Control-Allow-Origin: http://192.168.1.20:4000\r\n"),
        "{text}"
    );
}
