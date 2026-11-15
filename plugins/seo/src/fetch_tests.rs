use rb_plugin_api::test_support::MemoryHost;
use rb_plugin_api::{Capability, HttpResponse};

use super::*;

fn http_host() -> MemoryHost {
    MemoryHost::new().grant(Capability::Http)
}

fn redirect(to: &str) -> HttpResponse {
    HttpResponse {
        status: 301,
        headers: vec![("Location".to_owned(), to.to_owned())],
        body: Vec::new(),
    }
}

fn page(body: &str) -> HttpResponse {
    HttpResponse {
        status: 200,
        headers: vec![(
            "Content-Type".to_owned(),
            "text/html; charset=utf-8".to_owned(),
        )],
        body: body.as_bytes().to_vec(),
    }
}

fn url(text: &str) -> Url {
    Url::parse(text).expect("url")
}

#[test]
fn a_chain_is_followed_and_recorded() {
    let mut host = http_host();
    host.responses
        .insert("http://a.test/".to_owned(), redirect("https://a.test/"));
    host.responses
        .insert("https://a.test/".to_owned(), redirect("/de/"));
    host.responses
        .insert("https://a.test/de/".to_owned(), page("<html></html>"));
    let fetched = follow(&mut host, &url("http://a.test/"), MAX_HOPS);
    assert_eq!(
        fetched.hops,
        [
            ("http://a.test/".to_owned(), 301),
            ("https://a.test/".to_owned(), 301)
        ]
    );
    let response = fetched.response.expect("answer");
    assert_eq!(response.url, "https://a.test/de/");
    assert!(response.is_html());
    assert_eq!(
        response.header("content-type").next(),
        Some("text/html; charset=utf-8")
    );
    assert_eq!(host.fetched[0].headers[0].0, "accept");
}

#[test]
fn a_loop_and_a_long_chain_stop() {
    let mut host = http_host();
    host.responses
        .insert("https://a.test/1".to_owned(), redirect("/2"));
    host.responses
        .insert("https://a.test/2".to_owned(), redirect("/1"));
    let fetched = follow(&mut host, &url("https://a.test/1"), MAX_HOPS);
    assert_eq!(fetched.stop, Some(Stop::Loop));
    assert!(fetched.response.is_none());

    let mut host = http_host();
    for hop in 0..20 {
        host.responses.insert(
            format!("https://a.test/{hop}"),
            redirect(&format!("/{}", hop + 1)),
        );
    }
    let fetched = follow(&mut host, &url("https://a.test/0"), 3);
    assert_eq!(fetched.stop, Some(Stop::Loop));
    assert_eq!(host.fetched.len(), 4);
}

#[test]
fn a_public_page_never_redirects_into_the_local_network() {
    let mut host = http_host();
    host.responses.insert(
        "https://a.test/".to_owned(),
        redirect("http://192.168.0.1/admin"),
    );
    let fetched = follow(&mut host, &url("https://a.test/"), MAX_HOPS);
    assert_eq!(
        fetched.stop,
        Some(Stop::BadLocation("http://192.168.0.1/admin".to_owned()))
    );
    assert_eq!(host.fetched.len(), 1);
}

#[test]
fn a_refused_request_keeps_its_code() {
    let mut host = MemoryHost::new();
    let fetched = follow(&mut host, &url("https://a.test/"), MAX_HOPS);
    assert_eq!(
        fetched.stop,
        Some(Stop::Failed("plugins.error.capability_denied".to_owned()))
    );
}

#[test]
fn html_is_known_by_its_type_or_its_first_bytes() {
    let typed = Response {
        headers: vec![("content-type".to_owned(), "application/json".to_owned())],
        body: b"<html>".to_vec(),
        ..Response::default()
    };
    assert!(!typed.is_html());
    let sniffed = Response {
        body: b"  <!DOCTYPE html><html>".to_vec(),
        ..Response::default()
    };
    assert!(sniffed.is_html());
}

#[test]
fn a_request_sends_the_accept_header_it_is_given() {
    let mut host = http_host();
    host.responses
        .insert("https://a.test/a.png".to_owned(), redirect("/b.png"));
    let fetched = follow_with(&mut host, &url("https://a.test/a.png"), 3, ACCEPT_IMAGE);
    assert_eq!(fetched.hops.len(), 1);
    assert_eq!(host.fetched.len(), 2);
    for request in &host.fetched {
        assert_eq!(
            request.headers,
            [("accept".to_owned(), ACCEPT_IMAGE.to_owned())]
        );
    }
    let _ = follow(&mut host, &url("https://a.test/"), 1);
    assert_eq!(host.fetched[2].headers[0].1, ACCEPT);
}

#[test]
fn a_head_request_follows_its_redirects_as_head() {
    let mut host = http_host();
    host.responses
        .insert("https://a.test/doc".to_owned(), redirect("/doc.md"));
    let fetched = follow_method(
        &mut host,
        &url("https://a.test/doc"),
        3,
        "HEAD",
        ACCEPT_TEXT,
    );
    assert_eq!(fetched.response.map(|r| r.status), Some(404));
    assert!(host.fetched.iter().all(|request| request.method == "HEAD"));
}
