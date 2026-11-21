use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;

use rb_api_model::{Entry, Part};

use super::*;

fn no_file(path: &str) -> Result<Vec<u8>, Failure> {
    Ok(format!("<{path}>").into_bytes())
}

fn header<'a>(prepared: &'a Prepared, name: &str) -> Option<&'a str> {
    prepared
        .headers
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.as_str())
}

#[test]
fn the_auth_becomes_its_header_or_query() {
    let mut request = ApiRequest::new("GET", "https://a.example/x");
    request.auth = Auth::Basic {
        username: "ada".into(),
        password: "pw".into(),
    };
    let prepared = prepare(&request, &no_file).expect("prepared");
    assert_eq!(header(&prepared, "Authorization"), Some("Basic YWRhOnB3"));
    request.auth = Auth::ApiKey {
        name: "api key".into(),
        value: "a&b".into(),
        place: KeyPlace::Query,
    };
    request.url = "https://a.example/x?y=1#top".into();
    let prepared = prepare(&request, &no_file).expect("prepared");
    assert_eq!(prepared.url, "https://a.example/x?y=1&api+key=a%26b#top");
    request.auth = Auth::Digest {
        username: "u".into(),
        password: "p".into(),
    };
    let failure = prepare(&request, &no_file).expect_err("not yet");
    assert_eq!(
        failure.code(),
        Some("plugin.api-client.errors.auth_not_supported")
    );
}

#[test]
fn every_body_gets_its_bytes_and_default_type() {
    let mut request = ApiRequest::new("POST", "http://a.example");
    request.body = Body::Json {
        text: "{\"a\": 1}".into(),
    };
    let prepared = prepare(&request, &no_file).expect("prepared");
    assert_eq!(header(&prepared, "content-type"), Some("application/json"));
    request.body = Body::Json {
        text: "{broken".into(),
    };
    let failure = prepare(&request, &no_file).expect_err("refused");
    assert_eq!(
        failure.code(),
        Some("plugin.api-client.errors.invalid_json")
    );
    request.body = Body::Form {
        fields: vec![
            Entry::new("q", "a b&c"),
            Entry {
                name: "off".into(),
                value: "1".into(),
                enabled: false,
            },
        ],
    };
    let prepared = prepare(&request, &no_file).expect("prepared");
    assert_eq!(prepared.body, b"q=a+b%26c");
    request.body = Body::GraphQl {
        query: "{ me }".into(),
        variables: "{\"x\": 1}".into(),
    };
    let prepared = prepare(&request, &no_file).expect("prepared");
    let sent: Value = serde_json::from_slice(&prepared.body).expect("json");
    assert_eq!(sent, json!({ "query": "{ me }", "variables": { "x": 1 } }));
    request.body = Body::Binary {
        path: "./blob.bin".into(),
    };
    request
        .headers
        .push(Entry::new("Content-Type", "application/octet-stream"));
    let prepared = prepare(&request, &no_file).expect("prepared");
    assert_eq!(prepared.body, b"<./blob.bin>");
    assert_eq!(prepared.headers.len(), 1);
}

#[test]
fn a_multipart_body_names_its_boundary() {
    let mut request = ApiRequest::new("POST", "http://a.example");
    request.body = Body::Multipart {
        parts: vec![
            Part {
                name: "title".into(),
                value: "Logo".into(),
                file: None,
                content_type: None,
            },
            Part {
                name: "image".into(),
                value: String::new(),
                file: Some("./img/logo.png".into()),
                content_type: Some("image/png".into()),
            },
        ],
    };
    let prepared = prepare(&request, &no_file).expect("prepared");
    assert_eq!(
        header(&prepared, "Content-Type"),
        Some("multipart/form-data; boundary=rbrowser-api-client-boundary")
    );
    let text = String::from_utf8(prepared.body).expect("utf8");
    assert!(text.starts_with("--rbrowser-api-client-boundary\r\nContent-Disposition: form-data; name=\"title\"\r\n\r\nLogo\r\n"));
    assert!(text.contains("name=\"image\"; filename=\"logo.png\"\r\nContent-Type: image/png\r\n\r\n<./img/logo.png>\r\n"));
    assert!(text.ends_with("--rbrowser-api-client-boundary--\r\n"));
}

#[test]
fn only_http_and_https_are_sent() {
    let request = ApiRequest::new("GET", "file:///etc/passwd");
    let failure = prepare(&request, &no_file).expect_err("refused");
    assert_eq!(
        failure.code(),
        Some("plugin.api-client.errors.invalid_request")
    );
}

/// A server on 127.0.0.1 that answers one request with `answer` and hands back what it read.
fn serve_once(answer: &'static [u8]) -> (String, thread::JoinHandle<Vec<u8>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let url = format!("http://{}", listener.local_addr().expect("address"));
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept");
        let mut read = Vec::new();
        let mut chunk = [0u8; 4096];
        // The head, then as many body bytes as it announces.
        while !read.windows(4).any(|w| w == b"\r\n\r\n") {
            let n = stream.read(&mut chunk).expect("read");
            if n == 0 {
                break;
            }
            read.extend_from_slice(&chunk[..n]);
        }
        let text = String::from_utf8_lossy(&read).to_ascii_lowercase();
        let head = text.find("\r\n\r\n").map_or(read.len(), |at| at + 4);
        let length = text
            .lines()
            .find_map(|line| line.strip_prefix("content-length:"))
            .and_then(|value| value.trim().parse::<usize>().ok())
            .unwrap_or(0);
        while read.len() < head + length {
            let n = stream.read(&mut chunk).expect("read");
            if n == 0 {
                break;
            }
            read.extend_from_slice(&chunk[..n]);
        }
        stream.write_all(answer).expect("write");
        read
    });
    (url, handle)
}

#[test]
fn a_request_goes_out_and_its_answer_comes_back_timed_and_masked() {
    let (url, server) = serve_once(
        b"HTTP/1.1 201 Created\r\nContent-Type: application/json\r\nContent-Length: 27\r\nConnection: close\r\n\r\n{\"echo\": \"Bearer tok-9999\"}",
    );
    let mut request = ApiRequest::new("POST", &format!("{url}/users"));
    request.auth = Auth::Bearer {
        token: "tok-9999".into(),
    };
    request.body = Body::Json { text: "{}".into() };
    let prepared = prepare(&request, &no_file).expect("prepared");
    let secrets = vec!["tok-9999".to_owned()];
    let exchange = send_blocking(&prepared, 1024, &secrets, Route::Direct).expect("answered");
    let seen = String::from_utf8(server.join().expect("server")).expect("utf8");
    assert!(seen.starts_with("POST /users HTTP/1.1\r\n"));
    assert!(
        seen.contains("authorization: Bearer tok-9999")
            || seen.contains("Authorization: Bearer tok-9999")
    );
    assert_eq!(exchange.status, 201);
    assert_eq!(exchange.status_text, "Created");
    assert!(!exchange.truncated);
    assert!(exchange.phases.total >= exchange.phases.first_byte);
    let answer = answer(&prepared, &exchange, &secrets);
    assert_eq!(answer["status"], 201);
    assert_eq!(answer["body"], "{\"echo\": \"Bearer ***\"}");
    assert_eq!(answer["bodyEncoding"], "text");
    assert_eq!(answer["contentType"], "application/json");
}

#[test]
fn a_body_over_the_limit_is_cut_and_said_so() {
    let (url, server) = serve_once(
        b"HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: 6\r\nConnection: close\r\n\r\n\xff\xfe\x00abc",
    );
    let prepared = prepare(&ApiRequest::new("GET", &url), &no_file).expect("prepared");
    let exchange = send_blocking(&prepared, 4, &[], Route::Direct).expect("answered");
    drop(server.join());
    assert!(exchange.truncated);
    assert_eq!(exchange.body, b"\xff\xfe\x00a");
    let answer = answer(&prepared, &exchange, &[]);
    assert_eq!(answer["bodyEncoding"], "base64");
    assert_eq!(answer["size"], 4);
}

#[test]
fn a_refused_connection_fails_with_its_reason() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let url = format!("http://{}/", listener.local_addr().expect("address"));
    drop(listener);
    let prepared = prepare(&ApiRequest::new("GET", &url), &no_file).expect("prepared");
    let failure = send_blocking(&prepared, 1024, &[], Route::Direct).expect_err("refused");
    assert_eq!(failure.code(), Some("plugin.api-client.errors.send_failed"));
}

#[test]
fn form_encoding_keeps_the_unreserved_characters() {
    assert_eq!(form_encode("a-b_c.d~e f/\u{e9}"), "a-b_c.d~e+f%2F%C3%A9");
}

#[test]
fn a_basic_pair_holding_a_secret_is_a_secret_of_its_own() {
    let mut request = ApiRequest::new("GET", "https://a.example/x");
    request.auth = Auth::Basic {
        username: "ada".into(),
        password: "pw-secret".into(),
    };
    let secrets = vec!["pw-secret".to_owned()];
    assert_eq!(
        derived_secrets(&request, &secrets),
        ["YWRhOnB3LXNlY3JldA=="]
    );
    // Without a secret in it the pair is the person's own text, not masked.
    assert!(derived_secrets(&request, &[]).is_empty());
    request.auth = Auth::Bearer {
        token: "pw-secret".into(),
    };
    assert!(derived_secrets(&request, &secrets).is_empty());
}

fn redirect(status: u16, location: &str) -> Exchange {
    Exchange {
        status,
        status_text: String::new(),
        headers: vec![("Location".to_owned(), location.to_owned())],
        body: Vec::new(),
        truncated: false,
        phases: Phases::default(),
    }
}

#[test]
fn a_redirect_leads_on_as_a_browser_follows_it() {
    let mut request = ApiRequest::new("POST", "https://a.example/login");
    request.auth = Auth::Bearer {
        token: "t-1".into(),
    };
    request.body = Body::Json { text: "{}".into() };
    let prepared = prepare(&request, &no_file).expect("prepared");
    let same = next_hop(&prepared, &redirect(307, "/v2/login")).expect("follows");
    assert_eq!(same.url, "https://a.example/v2/login");
    assert_eq!(same.method, "POST");
    assert_eq!(same.body, b"{}");
    assert_eq!(header(&same, "Authorization"), Some("Bearer t-1"));
    let other = next_hop(&prepared, &redirect(303, "https://b.example/done")).expect("follows");
    assert_eq!(other.method, "GET");
    assert!(other.body.is_empty());
    assert_eq!(header(&other, "Authorization"), None);
    assert_eq!(header(&other, "Content-Type"), None);
    assert!(next_hop(&prepared, &redirect(200, "/x")).is_none());
    assert!(next_hop(&prepared, &redirect(302, "javascript:alert(1)")).is_none());
    let mut stay = prepared;
    stay.options.follow_redirects = false;
    assert!(next_hop(&stay, &redirect(302, "/x")).is_none());
}

#[test]
fn a_redirect_to_another_origin_takes_no_credential() {
    let mut request = ApiRequest::new("GET", "https://a.example/x");
    request.auth = Auth::ApiKey {
        name: "X-Api-Key".into(),
        value: "k-1".into(),
        place: KeyPlace::Header,
    };
    request
        .headers
        .push(Entry::new("Proxy-Authorization", "Basic cDpx"));
    request
        .headers
        .push(Entry::new("Authorization", "Bearer t-1"));
    request
        .headers
        .push(Entry::new("Accept", "application/json"));
    let prepared = prepare(&request, &no_file).expect("prepared");
    let same = next_hop(&prepared, &redirect(302, "https://a.example/y")).expect("follows");
    assert_eq!(header(&same, "X-Api-Key"), Some("k-1"));
    assert_eq!(header(&same, "Authorization"), Some("Bearer t-1"));
    // Another host, another port, and https down to http on the same host: another origin.
    for location in [
        "https://b.example/y",
        "https://a.example:8443/y",
        "http://a.example/y",
    ] {
        let hop = next_hop(&prepared, &redirect(302, location)).expect("follows");
        for name in ["X-Api-Key", "Authorization", "Proxy-Authorization"] {
            assert_eq!(header(&hop, name), None, "{name} to {location}");
        }
        assert_eq!(header(&hop, "Accept"), Some("application/json"));
    }
}

#[test]
fn a_header_holding_a_secret_stays_with_its_origin() {
    let mut request = ApiRequest::new("GET", "https://a.example/x");
    request.headers.push(Entry::new("X-Token", "s-1"));
    let mut prepared = prepare(&request, &no_file).expect("prepared");
    mark_secret_headers(&mut prepared, &["s-1".to_owned()]);
    assert_eq!(prepared.credentials, ["x-token"]);
    let hop = next_hop(&prepared, &redirect(307, "https://b.example/")).expect("follows");
    assert_eq!(header(&hop, "X-Token"), None);
}
