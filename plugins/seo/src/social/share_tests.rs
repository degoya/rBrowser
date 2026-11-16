use rb_plugin_api::test_support::MemoryHost;
use rb_plugin_api::{Capability, HttpResponse};

use super::*;
use crate::social::image::Format;
use crate::social::image::tests::png;

fn host() -> MemoryHost {
    MemoryHost::new().grant(Capability::Http)
}

fn image(kind: &str, body: Vec<u8>) -> HttpResponse {
    HttpResponse {
        status: 200,
        headers: vec![("Content-Type".to_owned(), kind.to_owned())],
        body,
    }
}

fn url(text: &str) -> Url {
    Url::parse(text).expect("url")
}

#[test]
fn an_image_is_read_after_its_redirect() {
    let mut host = host();
    host.responses.insert(
        "https://a.test/share".to_owned(),
        HttpResponse {
            status: 302,
            headers: vec![("location".to_owned(), "/share.png".to_owned())],
            body: Vec::new(),
        },
    );
    host.responses.insert(
        "https://a.test/share.png".to_owned(),
        image("image/png", png(1200, 630)),
    );
    let loaded = load(&mut host, &url("https://a.test/share"));
    assert!(loaded.ok() && loaded.is_image());
    assert_eq!(loaded.url, "https://a.test/share.png");
    let sniffed = loaded.image.expect("image");
    assert_eq!(
        (sniffed.format, sniffed.size),
        (Format::Png, Some((1200, 630)))
    );
    assert!(host.fetched.iter().all(|r| r.headers[0].1 == ACCEPT_IMAGE));
}

#[test]
fn html_served_as_an_image_is_none_and_a_missing_one_has_its_status() {
    let mut host = host();
    host.responses.insert(
        "https://a.test/x.png".to_owned(),
        image("text/html", b"<!doctype html><p>404</p>".to_vec()),
    );
    let loaded = load(&mut host, &url("https://a.test/x.png"));
    assert!(loaded.ok() && !loaded.is_image());
    let missing = load(&mut host, &url("https://a.test/none.png"));
    assert_eq!(missing.status, Some(404));
    assert!(!missing.ok());
}
