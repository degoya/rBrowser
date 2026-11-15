use std::io::{Read, Write};
use std::net::TcpListener;

use super::*;

fn request(method: &str, url: &str) -> ComposeRequest {
    ComposeRequest {
        method: method.to_owned(),
        url: url.to_owned(),
        headers: Vec::new(),
        body: None,
    }
}

#[test]
fn the_views_context_adds_cookies_user_agent_and_extra_headers_unless_set() {
    let context = ViewContext {
        cookies: vec![
            ("sid".to_owned(), "1".to_owned()),
            ("theme".to_owned(), "dark".to_owned()),
        ],
        user_agent: Some("UA/1".to_owned()),
        headers: vec![("X-Env".to_owned(), "staging".to_owned())],
    };
    let plain = with_context(&request("GET", "https://a.test/"), &context);
    assert_eq!(
        plain.headers,
        vec![
            ("Cookie".to_owned(), "sid=1; theme=dark".to_owned()),
            ("User-Agent".to_owned(), "UA/1".to_owned()),
            ("X-Env".to_owned(), "staging".to_owned()),
        ]
    );
    let mut own = request("GET", "https://a.test/");
    own.headers = vec![
        ("cookie".to_owned(), "mine=1".to_owned()),
        ("user-agent".to_owned(), "Me".to_owned()),
        ("x-env".to_owned(), "prod".to_owned()),
    ];
    assert_eq!(
        with_context(&own, &context).headers,
        own.headers,
        "the caller's win"
    );
    let empty = with_context(&request("GET", "https://a.test/"), &ViewContext::default());
    assert!(empty.headers.is_empty());
}

#[test]
fn edits_change_method_url_headers_and_body() {
    let mut original = request("GET", "https://a.test/x");
    original.headers = vec![("Accept".to_owned(), "*/*".to_owned())];
    let edits = Edits {
        method: Some("POST".to_owned()),
        url: None,
        headers: None,
        body: Some(b"a=1".to_vec()),
        status: Some(500),
    };
    let edited = with_edits(&original, &edits);
    assert_eq!(edited.method, "POST");
    assert_eq!(edited.url, "https://a.test/x");
    assert_eq!(
        edited.headers, original.headers,
        "kept when the edits name none"
    );
    assert_eq!(edited.body.as_deref(), Some(b"a=1".as_slice()));
}

/// A server answering one request with `response`, giving back what it read.
fn serve(response: &'static str) -> (String, std::thread::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a port");
    let port = listener.local_addr().expect("an address").port();
    let handle = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("a connection");
        // The headers and the body may arrive in separate reads.
        let mut seen = Vec::new();
        let mut chunk = vec![0u8; 8192];
        loop {
            let read = stream.read(&mut chunk).expect("reads");
            seen.extend_from_slice(&chunk[..read]);
            let text = String::from_utf8_lossy(&seen).into_owned();
            let Some((head, body)) = text.split_once("\r\n\r\n") else {
                continue;
            };
            let length = head
                .lines()
                .find_map(|line| {
                    line.to_lowercase()
                        .strip_prefix("content-length:")
                        .map(|n| n.trim().parse::<usize>().unwrap_or(0))
                })
                .unwrap_or(0);
            if body.len() >= length || read == 0 {
                break;
            }
        }
        stream.write_all(response.as_bytes()).expect("writes");
        String::from_utf8_lossy(&seen).into_owned()
    });
    (format!("http://127.0.0.1:{port}/api?x=1"), handle)
}

#[test]
fn a_request_is_sent_with_its_headers_and_body_and_the_answer_read() {
    let (url, server) = serve(
        "HTTP/1.1 201 Created\r\nContent-Type: text/plain\r\nSet-Cookie: s=1\r\nContent-Length: 5\r\n\r\nhello",
    );
    let mut composed = request("POST", &url);
    composed.headers = vec![
        ("Cookie".to_owned(), "sid=1".to_owned()),
        ("Content-Type".to_owned(), "text/plain".to_owned()),
    ];
    composed.body = Some(b"ping".to_vec());
    let result = send(&agent(), &composed, 1024, Some(ViewId(3)));
    let seen = server.join().expect("the server's read");
    assert!(seen.starts_with("POST /api?x=1 HTTP/1.1\r\n"), "{seen}");
    assert!(seen.to_lowercase().contains("cookie: sid=1"), "{seen}");
    assert!(seen.ends_with("ping"), "{seen}");
    assert_eq!(result.status, Some(201));
    assert_eq!(result.error, None);
    assert_eq!(result.body, b"hello");
    assert!(!result.body_truncated);
    assert!(
        result
            .headers
            .contains(&("set-cookie".to_owned(), "s=1".to_owned()))
    );
    assert_eq!(result.view, Some(ViewId(3)));
    let json = result.to_json(true);
    assert_eq!(json["status"], 201);
    assert_eq!(json["body"], "hello");
    assert_eq!(json["bodyBase64"], false);
    let masked: Vec<_> = json["headers"]
        .as_array()
        .expect("headers")
        .iter()
        .filter(|pair| pair[0] == "set-cookie")
        .collect();
    assert_eq!(masked[0][1], "***");
    assert_eq!(
        result.to_json(false)["headers"]
            .as_array()
            .expect("headers")
            .iter()
            .find(|pair| pair[0] == "set-cookie")
            .map(|pair| pair[1].clone()),
        Some(json!("s=1"))
    );
}

#[test]
fn a_large_body_is_cut_and_a_refused_connection_is_an_error() {
    let (url, server) = serve("HTTP/1.1 200 OK\r\nContent-Length: 10\r\n\r\n0123456789");
    let result = send(&agent(), &request("GET", &url), 4, None);
    server.join().expect("served");
    assert_eq!(result.body, b"0123");
    assert!(result.body_truncated);
    let listener = TcpListener::bind("127.0.0.1:0").expect("a port");
    let closed = format!(
        "http://127.0.0.1:{}/",
        listener.local_addr().expect("an address").port()
    );
    drop(listener);
    let failed = send(&agent(), &request("GET", &closed), 4, None);
    assert_eq!(failed.status, None);
    assert!(failed.error.is_some());
    assert!(failed.to_json(true)["error"].is_string());
}

#[test]
fn a_cookie_header_joins_name_and_value_pairs() {
    assert_eq!(
        cookie_header(&[
            ("a".to_owned(), "1".to_owned()),
            ("b".to_owned(), "2".to_owned())
        ]),
        "a=1; b=2"
    );
}

#[test]
fn a_redirect_is_answered_as_it_came_and_not_followed() {
    let (url, server) =
        serve("HTTP/1.1 302 Found\r\nLocation: http://other.test/\r\nContent-Length: 0\r\n\r\n");
    let mut request = request("GET", &url);
    request.headers = vec![("Cookie".to_owned(), "sid=1".to_owned())];
    let result = send(&agent(), &request, 1024, None);
    server.join().expect("served");
    assert_eq!(result.status, Some(302));
    assert_eq!(result.error, None);
    assert!(
        result
            .headers
            .iter()
            .any(|(name, value)| name == "location" && value == "http://other.test/")
    );
}

#[test]
fn a_repeat_resends_the_captured_headers_but_the_hop_and_length_ones() {
    let headers = vec![
        ("Content-Type".to_owned(), "application/json".to_owned()),
        ("Host".to_owned(), "a.test".to_owned()),
        ("content-length".to_owned(), "3".to_owned()),
        ("Connection".to_owned(), "keep-alive".to_owned()),
        ("X-Custom".to_owned(), "1".to_owned()),
    ];
    assert_eq!(
        resendable(&headers),
        vec![
            ("Content-Type".to_owned(), "application/json".to_owned()),
            ("X-Custom".to_owned(), "1".to_owned()),
        ]
    );
}

#[test]
fn a_repeat_moved_to_another_origin_leaves_the_captured_secrets_behind() {
    let mut original = request("GET", "https://a.test/api");
    original.headers = vec![
        ("Authorization".to_owned(), "Bearer t".to_owned()),
        ("X-CSRF-Token".to_owned(), "c".to_owned()),
        ("Referer".to_owned(), "https://a.test/".to_owned()),
        ("Accept".to_owned(), "*/*".to_owned()),
    ];
    let moved = Edits {
        url: Some("https://evil.test/c".to_owned()),
        ..Edits::default()
    };
    assert_eq!(
        with_edits(&original, &moved).headers,
        vec![("Accept".to_owned(), "*/*".to_owned())]
    );
    let same = Edits {
        url: Some("https://a.test/other".to_owned()),
        ..Edits::default()
    };
    assert_eq!(with_edits(&original, &same).headers, original.headers);
}
