use super::*;

const UPGRADE: &str = "GET /devtools/browser/b HTTP/1.1\r\nHost: 127.0.0.1:9222\r\n\
Upgrade: websocket\r\nConnection: keep-alive, Upgrade\r\nSec-WebSocket-Version: 13\r\n\
Sec-WebSocket-Key: a2V5\r\n\r\n";

#[test]
fn the_head_ends_after_the_blank_line() {
    assert_eq!(head_end(b"GET / HTTP/1.1\r\nHost: x\r\n\r\nrest"), Some(27));
    assert_eq!(head_end(b"GET / HTTP/1.1\r\nHost: x\r\n"), None);
}

#[test]
fn a_head_parses_into_method_target_and_headers() {
    let head = parse_head(b"GET /json/list?x=1 HTTP/1.1\r\nHost: localhost:9\r\nX-A:  b \r\n\r\n")
        .expect("parsed");
    assert_eq!(head.method, "GET");
    assert_eq!(head.target, "/json/list?x=1");
    assert_eq!(head.path(), "/json/list");
    assert_eq!(head.header("host"), Some("localhost:9"));
    assert_eq!(head.header("x-a"), Some("b"));
    assert_eq!(head.header("missing"), None);
}

#[test]
fn malformed_heads_are_refused() {
    for bad in [
        &b"GET /\r\n\r\n"[..],
        b"GET / HTTP/1.1 extra\r\n\r\n",
        b"GET relative HTTP/1.1\r\n\r\n",
        b"GET / FTP/1.0\r\n\r\n",
        b"GET / HTTP/1.1\r\nNoColon\r\n\r\n",
        b"\xff\xfe / HTTP/1.1\r\n\r\n",
    ] {
        assert_eq!(parse_head(bad), Err(HeadError::Malformed), "{bad:?}");
    }
}

#[test]
fn a_head_past_the_limit_is_too_large() {
    let mut big = b"GET / HTTP/1.1\r\nX: ".to_vec();
    big.extend(std::iter::repeat_n(b'a', MAX_HEAD));
    big.extend(b"\r\n\r\n");
    assert_eq!(parse_head(&big), Err(HeadError::TooLarge));
}

#[test]
fn a_websocket_upgrade_gives_its_key() {
    let head = parse_head(UPGRADE.as_bytes()).expect("parsed");
    assert_eq!(websocket_key(&head), Some("a2V5"));
}

#[test]
fn a_plain_get_or_an_incomplete_upgrade_gives_no_key() {
    let plain = parse_head(b"GET /json HTTP/1.1\r\nHost: x\r\n\r\n").expect("parsed");
    assert_eq!(websocket_key(&plain), None);
    let old = UPGRADE.replace("Version: 13", "Version: 8");
    assert_eq!(
        websocket_key(&parse_head(old.as_bytes()).expect("parsed")),
        None
    );
    let no_connection = UPGRADE.replace("keep-alive, Upgrade", "keep-alive");
    assert_eq!(
        websocket_key(&parse_head(no_connection.as_bytes()).expect("parsed")),
        None
    );
    let post = UPGRADE.replace("GET", "POST");
    assert_eq!(
        websocket_key(&parse_head(post.as_bytes()).expect("parsed")),
        None
    );
}
