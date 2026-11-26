use std::io::{Read as _, Write as _};
use std::net::{SocketAddr, TcpListener};
use std::sync::{Arc, RwLock};

use rb_plugin_types::{HttpRequest, codes};
use ureq::http::Uri;

use super::{RESPONSE_HEADERS, admitted, agent, fetch_blocking, passed_value};
use crate::plugins::services::net::Origin;

fn addresses(list: &[&str]) -> Vec<SocketAddr> {
    list.iter()
        .map(|text| text.parse().expect("an address"))
        .collect()
}

fn uri(text: &str) -> Uri {
    text.parse().expect("a URI")
}

#[test]
fn a_name_outside_the_project_reaches_only_its_public_addresses() {
    let mixed = addresses(&["10.0.0.5:443", "93.184.216.34:443", "[::1]:443"]);
    let kept = admitted(&uri("https://site.test/"), &mixed, &[]).expect("one public");
    assert_eq!(&kept[..], &addresses(&["93.184.216.34:443"])[..]);

    let internal = addresses(&["127.0.0.1:80", "169.254.169.254:80"]);
    let refused = admitted(&uri("http://metadata.test/"), &internal, &[]).expect_err("none");
    assert!(matches!(refused, ureq::Error::Other(_)));
}

#[test]
fn the_projects_own_origin_may_be_local() {
    let local = vec![Origin::of("http://localhost:3000/").expect("origin")];
    let loopback = addresses(&["127.0.0.1:3000", "[::1]:3000"]);
    let kept = admitted(&uri("http://localhost:3000/robots.txt"), &loopback, &local)
        .expect("the project's own");
    assert_eq!(kept.len(), 2);
    // Another port of the same host is another origin.
    let other = addresses(&["127.0.0.1:5432"]);
    assert!(admitted(&uri("http://localhost:5432/"), &other, &local).is_err());
}

#[test]
fn a_refused_address_is_the_url_denied_code_and_nothing_connects() {
    let agent = agent(std::sync::Arc::default());
    let refused = fetch_blocking(
        &agent,
        &HttpRequest {
            method: "GET".to_owned(),
            url: "http://127.0.0.1:9/".to_owned(),
            ..HttpRequest::default()
        },
        1024,
    )
    .expect_err("loopback");
    assert_eq!(refused.code(), Some(codes::HTTP_URL_DENIED));
}

#[test]
fn the_security_headers_reach_the_plugins_and_never_a_cookie_value() {
    for name in [
        "content-security-policy",
        "strict-transport-security",
        "x-frame-options",
        "x-content-type-options",
        "referrer-policy",
        "permissions-policy",
        "access-control-allow-origin",
        "access-control-allow-credentials",
        "date",
        "set-cookie",
    ] {
        assert!(RESPONSE_HEADERS.contains(&name), "{name}");
    }
    assert!(!RESPONSE_HEADERS.contains(&"cookie"));
    assert_eq!(
        passed_value(
            "set-cookie",
            "sid=SECRET123; Path=/; Secure; HttpOnly; SameSite=Lax"
        ),
        "sid=; Path=/; Secure; HttpOnly; SameSite=Lax"
    );
    assert_eq!(passed_value("set-cookie", "token=a=b=c"), "token=");
    assert_eq!(passed_value("set-cookie", "SECRETONLY"), "");
    assert_eq!(
        passed_value("content-security-policy", "default-src 'self'"),
        "default-src 'self'"
    );
}

#[test]
fn a_set_cookie_passes_only_its_name_and_known_attributes() {
    // A folded second cookie, a quoted value with `;` and an unknown token leak nothing.
    assert_eq!(
        passed_value("set-cookie", "a=1; Path=/, b=SECRET; Secure"),
        "a=; Secure"
    );
    assert_eq!(
        passed_value("set-cookie", "sid=\"x;SECRET\"; HttpOnly"),
        "sid=; HttpOnly"
    );
    assert_eq!(
        passed_value(
            "set-cookie",
            "id=1; Expires=Thu, 01 Jan 2030 00:00:00 GMT; Max-Age=3600; Domain=example.test; SameSite=Lax; Partitioned; X-Secret=abc"
        ),
        "id=; Expires=Thu, 01 Jan 2030 00:00:00 GMT; Max-Age=3600; Domain=example.test; SameSite=Lax; Partitioned"
    );
    assert_eq!(
        passed_value(
            "set-cookie",
            "id=1; Expires=Thu, 01 Jan 2030 00:00:00 GMT, b=SECRET"
        ),
        "id="
    );
}

/// A server on 127.0.0.1 that answers each connection with `answer` and hands the request it
/// read back, and an agent that admits it as the project's own origin.
fn server(answer: &'static str) -> (String, ureq::Agent, std::sync::mpsc::Receiver<String>) {
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
    let local = vec![Origin::of(&format!("{origin}/")).expect("origin")];
    (origin, agent(Arc::new(RwLock::new(local))), receiver)
}

fn get(url: &str, headers: &[(&str, &str)]) -> HttpRequest {
    HttpRequest {
        method: "GET".to_owned(),
        url: url.to_owned(),
        headers: headers
            .iter()
            .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
            .collect(),
        body: Vec::new(),
    }
}

/// The redirect leads to a port nothing listens on, outside the project's origins: followed, the
/// fetch would fail, so an answer with the 301 itself is the proof - no timing involved.
#[test]
fn a_redirect_is_answered_not_followed() {
    let (origin, agent, _requests) = server(
        "HTTP/1.1 301 Moved\r\nLocation: http://127.0.0.1:9/elsewhere\r\nContent-Length: 0\r\n\r\n",
    );
    let answer =
        fetch_blocking(&agent, &get(&format!("{origin}/sitemap.xml"), &[]), 1024).expect("answers");
    assert_eq!(answer.status, 301);
    assert!(answer.headers.contains(&(
        "location".to_owned(),
        "http://127.0.0.1:9/elsewhere".to_owned()
    )));
}

#[test]
fn only_the_allowed_headers_go_out_and_the_body_is_capped() {
    let (origin, agent, requests) = server("HTTP/1.1 200 OK\r\nContent-Length: 2000\r\n\r\n");
    let request = get(
        &format!("{origin}/big"),
        &[
            ("Accept", "application/xml"),
            ("Cookie", "secret=1"),
            ("Authorization", "Bearer x"),
        ],
    );
    let failure = fetch_blocking(&agent, &request, 100).expect_err("too large");
    assert_eq!(failure.code(), Some(codes::TOO_LARGE));
    let sent = requests.recv().expect("a request").to_ascii_lowercase();
    assert!(sent.contains("accept: application/xml"));
    assert!(!sent.contains("cookie"));
    assert!(!sent.contains("authorization"));
}

#[test]
fn other_methods_are_refused() {
    let mut request = get("http://127.0.0.1:9/", &[]);
    request.method = "POST".to_owned();
    let failure = fetch_blocking(&agent(Arc::default()), &request, 100).expect_err("refused");
    assert_eq!(failure.code(), Some(codes::HTTP_FAILED));
}

/// No cookie store: ureq's `cookies` feature is not enabled in the workspace.
#[test]
fn a_cookie_the_server_sets_never_goes_back() {
    let (origin, agent, requests) =
        server("HTTP/1.1 200 OK\r\nSet-Cookie: session=abc; Path=/\r\nContent-Length: 0\r\n\r\n");
    for path in ["robots.txt", "sitemap.xml"] {
        fetch_blocking(&agent, &get(&format!("{origin}/{path}"), &[]), 1024).expect("answers");
    }
    let first = requests.recv().expect("a request").to_ascii_lowercase();
    let second = requests.recv().expect("a request").to_ascii_lowercase();
    assert!(second.contains("get /sitemap.xml"), "{second}");
    for sent in [first, second] {
        assert!(!sent.contains("cookie"), "{sent}");
    }
}
