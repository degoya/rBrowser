use std::time::Duration;

use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use super::*;

#[test]
fn a_request_goes_to_the_loopback_port_with_its_json_body() {
    let bytes = request_bytes(Method::Post, 4444, "/session", Some(&json!({ "a": 1 })));
    let text = String::from_utf8(bytes).expect("ASCII");
    assert!(text.starts_with("POST /session HTTP/1.1\r\n"), "{text}");
    assert!(text.contains("\r\nHost: 127.0.0.1:4444\r\n"), "{text}");
    assert!(text.contains("\r\nConnection: close\r\n"), "{text}");
    assert!(
        text.contains("\r\nContent-Type: application/json; charset=utf-8\r\n"),
        "{text}"
    );
    assert!(text.contains("\r\nContent-Length: 7\r\n"), "{text}");
    assert!(text.ends_with("\r\n\r\n{\"a\":1}"), "{text}");
    assert_eq!(loopback(4444).to_string(), "127.0.0.1:4444");
}

#[test]
fn a_get_has_no_body() {
    let text = String::from_utf8(request_bytes(Method::Get, 1, "/status", None)).expect("ASCII");
    assert!(text.starts_with("GET /status HTTP/1.1\r\n"));
    assert!(text.contains("\r\nContent-Length: 0\r\n"));
    assert!(!text.contains("Content-Type"));
    assert!(text.ends_with("\r\n\r\n"));
}

#[test]
fn an_answer_with_a_length_is_read_to_its_length() {
    let raw = b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\ncontent-length: 14\r\n\r\n{\"value\":null}";
    let answer = try_parse(raw, false).expect("valid").expect("complete");
    assert_eq!(answer.status, 200);
    assert_eq!(answer.body, json!({ "value": null }));
    // Half of it is not an answer yet, and an end there is an error.
    let half = &raw[..raw.len() - 4];
    assert_eq!(try_parse(half, false), Ok(None));
    assert!(try_parse(half, true).is_err());
}

#[test]
fn a_chunked_answer_is_joined() {
    let raw = b"HTTP/1.1 404 Not Found\r\nTransfer-Encoding: chunked\r\n\r\n5\r\n{\"val\r\n9;x=1\r\nue\":true}\r\n0\r\n\r\n";
    let answer = try_parse(raw, false).expect("valid").expect("complete");
    assert_eq!(answer.status, 404);
    assert_eq!(answer.body, json!({ "value": true }));
    assert_eq!(try_parse(&raw[..raw.len() - 5], false), Ok(None));
}

#[test]
fn an_answer_without_framing_ends_with_the_connection() {
    let raw = b"HTTP/1.1 200 OK\r\n\r\n{\"value\":1}";
    assert_eq!(try_parse(raw, false), Ok(None));
    let answer = try_parse(raw, true).expect("valid").expect("complete");
    assert_eq!(answer.body, json!({ "value": 1 }));
}

#[test]
fn an_empty_body_is_null() {
    let raw = b"HTTP/1.1 204 No Content\r\n\r\n";
    let answer = try_parse(raw, false).expect("valid").expect("complete");
    assert_eq!(answer.status, 204);
    assert_eq!(answer.body, serde_json::Value::Null);
}

#[test]
fn what_is_not_http_or_json_is_refused_without_quoting_it() {
    let not_http = try_parse(b"SSH-2.0-OpenSSH\r\n\r\n", true).expect_err("not HTTP");
    assert!(!not_http.to_string().contains("OpenSSH"), "{not_http}");
    let secret = b"HTTP/1.1 200 OK\r\nContent-Length: 12\r\n\r\nhunter2 text";
    let not_json = try_parse(secret, false).expect_err("not JSON");
    assert!(!not_json.to_string().contains("hunter2"), "{not_json}");
    let bad_chunk = try_parse(
        b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\nzz\r\n",
        false,
    );
    assert!(bad_chunk.is_err());
}

#[test]
fn a_chunk_size_that_overflows_is_refused_without_a_panic() {
    let raw = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\nffffffffffffffff\r\nab";
    assert!(try_parse(raw, false).is_err());
    assert!(try_parse(raw, true).is_err());
    let over_cap = format!(
        "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n{:x}\r\nab",
        MAX_ANSWER_BYTES + 1
    );
    assert!(try_parse(over_cap.as_bytes(), false).is_err());
}

#[test]
fn a_chunk_must_end_its_line_and_its_size_line_is_short() {
    let unterminated = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n2\r\n{}XX0\r\n\r\n";
    assert!(try_parse(unterminated, false).is_err());
    let mut endless = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n".to_vec();
    endless.extend(std::iter::repeat_n(b'1', 64 * 1024));
    assert!(
        try_parse(&endless, false).is_err(),
        "a size line never ends"
    );
}

#[test]
fn an_answer_fed_piece_by_piece_is_parsed_once_complete() {
    let chunked: &[u8] = b"HTTP/1.1 404 Not Found\r\nTransfer-Encoding: chunked\r\n\r\n5\r\n{\"val\r\n9;x=1\r\nue\":true}\r\n0\r\n\r\n";
    let length: &[u8] = b"HTTP/1.1 200 OK\r\nContent-Length: 14\r\n\r\n{\"value\":null}";
    for (raw, body) in [
        (chunked, json!({ "value": true })),
        (length, json!({ "value": null })),
    ] {
        let mut parser = AnswerParser::default();
        let mut answer = None;
        for (at, byte) in raw.iter().enumerate() {
            let parsed = parser
                .push(std::slice::from_ref(byte), false)
                .expect("valid so far");
            if at + 1 < raw.len() {
                assert_eq!(parsed, None, "complete only with its last byte ({at})");
            }
            answer = parsed;
        }
        assert_eq!(answer.expect("complete").body, body);
        assert_eq!(parser.len(), raw.len());
    }
}

#[test]
fn a_segment_of_a_path_is_plain() {
    assert!(is_plain_segment("6c1f0f2e-8a7b-4d1c-9b0a-1f2e3d4c5b6a"));
    assert!(is_plain_segment("node-1.2_3"));
    assert!(!is_plain_segment(""));
    assert!(!is_plain_segment("a b"));
    assert!(!is_plain_segment("a/../b"));
    assert!(!is_plain_segment("a\r\nHost: evil"));
    assert!(!is_plain_segment(&"x".repeat(200)));
}

#[tokio::test]
async fn a_request_reaches_a_server_on_loopback_and_reads_its_answer() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("a port");
    let port = listener.local_addr().expect("bound").port();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("a client");
        let mut seen = vec![0_u8; 4096];
        let read = stream.read(&mut seen).await.expect("the request");
        let body = b"{\"value\":{\"ready\":true}}";
        let head = format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n", body.len());
        stream.write_all(head.as_bytes()).await.expect("written");
        stream.write_all(body).await.expect("written");
        // The connection stays open: the length ends the answer, not the close.
        tokio::time::sleep(Duration::from_secs(30)).await;
        String::from_utf8_lossy(&seen[..read]).into_owned()
    });
    let answer = request(port, Method::Get, "/status", None, Duration::from_secs(10))
        .await
        .expect("an answer");
    assert_eq!(answer.body, json!({ "value": { "ready": true } }));
    server.abort();
}

#[tokio::test]
async fn nobody_on_the_port_is_a_connect_error() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("a port");
    let port = listener.local_addr().expect("bound").port();
    drop(listener);
    let error = request(port, Method::Get, "/status", None, Duration::from_secs(10))
        .await
        .expect_err("nobody listens");
    assert!(matches!(error, HttpError::Connect(_)), "{error:?}");
}

#[tokio::test(start_paused = true)]
async fn a_server_that_never_answers_times_out() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("a port");
    let port = listener.local_addr().expect("bound").port();
    let server = tokio::spawn(async move {
        let (_stream, _) = listener.accept().await.expect("a client");
        tokio::time::sleep(Duration::from_secs(3600)).await;
    });
    let error = request(port, Method::Get, "/status", None, Duration::from_secs(5))
        .await
        .expect_err("no answer");
    assert_eq!(error, HttpError::Timeout(5000));
    server.abort();
}

#[tokio::test]
async fn a_connection_closed_without_an_answer_is_a_broken_one() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("a port");
    let port = listener.local_addr().expect("bound").port();
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("a client");
        drop(stream);
    });
    let error = request(port, Method::Get, "/status", None, Duration::from_secs(10))
        .await
        .expect_err("no answer");
    assert!(matches!(error, HttpError::Io(_)), "{error:?}");
    server.abort();
}
