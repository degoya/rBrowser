use std::io::{Read as _, Write as _};
use std::net::TcpListener;

use super::*;

/// A server that answers each connection with `answer` and hands the request it read back.
fn server(answer: &'static str) -> (String, std::sync::mpsc::Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a port");
    let origin = format!("http://{}", listener.local_addr().expect("address"));
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            let mut request = Vec::new();
            let mut chunk = [0u8; 1024];
            while !request.windows(4).any(|end| end == b"\r\n\r\n") {
                match stream.read(&mut chunk) {
                    Ok(0) | Err(_) => break,
                    Ok(read) => request.extend_from_slice(&chunk[..read]),
                }
            }
            let _ = sender.send(String::from_utf8_lossy(&request).into_owned());
            let _ = stream.write_all(answer.as_bytes());
        }
    });
    (origin, receiver)
}

fn get(url: &str, headers: Vec<(&str, &str)>) -> HttpRequest {
    HttpRequest {
        method: "GET".to_owned(),
        url: url.to_owned(),
        headers: headers
            .into_iter()
            .map(|(name, value)| (name.to_owned(), value.to_owned()))
            .collect(),
        body: Vec::new(),
    }
}

#[tokio::test]
async fn a_redirect_is_answered_not_followed() {
    let (origin, _requests) = server(
        "HTTP/1.1 301 Moved\r\nLocation: /elsewhere\r\nSet-Cookie: a=b\r\nContent-Length: 0\r\n\r\n",
    );
    let answer = CoreServices::default()
        .fetch(get(&format!("{origin}/sitemap.xml"), vec![]), 1024)
        .await
        .expect("answers");
    assert_eq!(answer.status, 301);
    assert_eq!(
        answer.headers,
        [
            ("location".to_owned(), "/elsewhere".to_owned()),
            ("content-length".to_owned(), "0".to_owned())
        ],
        "only the headers a plugin needs; no set-cookie"
    );
}

#[tokio::test]
async fn only_the_allowed_headers_go_out_and_the_body_is_capped() {
    let (origin, requests) = server("HTTP/1.1 200 OK\r\nContent-Length: 2000\r\n\r\n");
    let request = get(
        &format!("{origin}/big"),
        vec![
            ("Accept", "application/xml"),
            ("Cookie", "secret=1"),
            ("Authorization", "Bearer x"),
        ],
    );
    let failure = CoreServices::default()
        .fetch(request, 100)
        .await
        .expect_err("too large");
    assert_eq!(failure.code(), Some(codes::TOO_LARGE));
    let sent = requests.recv().expect("a request").to_ascii_lowercase();
    assert!(sent.contains("accept: application/xml"));
    assert!(!sent.contains("cookie"));
    assert!(!sent.contains("authorization"));
}

#[tokio::test]
async fn other_methods_are_refused() {
    let mut request = get("http://127.0.0.1:9/", vec![]);
    request.method = "POST".to_owned();
    let failure = CoreServices::default()
        .fetch(request, 100)
        .await
        .expect_err("refused");
    assert_eq!(failure.code(), Some(codes::HTTP_FAILED));
}

#[tokio::test]
async fn a_cookie_the_server_sets_never_goes_back() {
    let (origin, requests) =
        server("HTTP/1.1 200 OK\r\nSet-Cookie: session=abc; Path=/\r\nContent-Length: 0\r\n\r\n");
    let services = CoreServices::default();
    for path in ["robots.txt", "sitemap.xml"] {
        services
            .fetch(get(&format!("{origin}/{path}"), vec![]), 1024)
            .await
            .expect("answers");
    }
    let first = requests.recv().expect("a request").to_ascii_lowercase();
    let second = requests.recv().expect("a request").to_ascii_lowercase();
    assert!(second.contains("get /sitemap.xml"), "{second}");
    for sent in [first, second] {
        assert!(!sent.contains("cookie"), "{sent}");
    }
}
