use rb_plugin_api::test_support::MemoryHost;
use rb_plugin_api::{Capability, HttpResponse};

use super::*;
use crate::social::image::tests::png;

const PAGE: &str = "https://shop.test/shoes/red/";

fn page() -> Url {
    Url::parse(PAGE).expect("a test URL")
}

fn picked(head: &str) -> (String, String) {
    let html = Html::parse_document(&format!("<html><head>{head}</head><body></body></html>"));
    let (source, url) = pick(Some(&html), &page());
    (source.to_owned(), url.to_string())
}

/// An ICO header: one entry of `size` (0 is 256) pixels.
fn ico(size: u8) -> Vec<u8> {
    let mut bytes = vec![0, 0, 1, 0, 1, 0, size, size, 0, 0, 1, 0, 32, 0];
    bytes.extend_from_slice(&[0; 8]);
    bytes
}

fn serve(url: &str, kind: &str, body: Vec<u8>) -> MemoryHost {
    let mut host = MemoryHost::new().grant(Capability::Http);
    host.responses.insert(
        url.to_owned(),
        HttpResponse {
            status: 200,
            headers: vec![("content-type".to_owned(), kind.to_owned())],
            body,
        },
    );
    host
}

#[test]
fn a_raster_icon_link_comes_first_resolved_against_the_page() {
    assert_eq!(
        picked(
            r#"<link rel="apple-touch-icon" href="/apple.png">
<link rel="icon" type="image/svg+xml" href="/icon.svg">
<link rel="shortcut icon" href="../fav.png">"#
        ),
        (
            "icon".to_owned(),
            "https://shop.test/shoes/fav.png".to_owned()
        )
    );
}

#[test]
fn the_apple_touch_icon_stands_in_for_a_missing_raster_icon() {
    assert_eq!(
        picked(
            r#"<link rel="icon" href="/icon.SVG?v=2"><link rel="apple-touch-icon-precomposed" href="/apple.png">"#
        ),
        (
            "apple-touch-icon".to_owned(),
            "https://shop.test/apple.png".to_owned()
        )
    );
}

#[test]
fn without_an_icon_link_it_is_the_site_s_favicon_ico() {
    let default = (
        "default".to_owned(),
        "https://shop.test/favicon.ico".to_owned(),
    );
    assert_eq!(picked(""), default);
    assert_eq!(
        picked(r#"<link rel="icon" href="javascript:alert(1)">"#),
        default
    );
    assert_eq!(
        picked(r#"<link rel="mask-icon" href="/mask.svg">"#),
        default
    );
    let (source, url) = pick(None, &page());
    assert_eq!((source, url.to_string()), ("default", default.1));
}

#[test]
fn an_ico_png_or_jpeg_sniffs_and_an_svg_or_text_does_not() {
    assert_eq!(sniff(&ico(32)), Some(("image/x-icon", Some((32, 32)))));
    assert_eq!(sniff(&ico(0)), Some(("image/x-icon", Some((256, 256)))));
    assert_eq!(sniff(&png(48, 48)), Some(("image/png", Some((48, 48)))));
    assert_eq!(
        sniff(b"<svg xmlns='http://www.w3.org/2000/svg'></svg>"),
        None
    );
    assert_eq!(sniff(b"<!doctype html>"), None);
    assert_eq!(sniff(&[0, 0, 1, 0, 0, 0]), None, "an ICO without an entry");
}

#[test]
fn a_favicon_loads_once_as_a_data_uri() {
    let url = "https://shop.test/favicon.ico";
    let mut host = serve(url, "image/vnd.microsoft.icon", ico(16));
    let (entry, data_uri) = load(&mut host, "default", &Url::parse(url).expect("url"));
    assert_eq!(host.fetched.len(), 1);
    assert_eq!(entry["source"], "default");
    assert_eq!(entry["status"], 200);
    assert_eq!(entry["format"], "image/x-icon");
    assert_eq!(
        (entry["width"].as_u64(), entry["height"].as_u64()),
        (Some(16), Some(16))
    );
    assert_eq!(
        entry["dataUri"],
        Value::Null,
        "the tool sets it within its budget"
    );
    assert!(data_uri.is_some_and(|uri| uri.starts_with("data:image/x-icon;base64,AAABAAEA")));
}

#[test]
fn a_favicon_over_its_limit_or_of_another_type_is_not_inlined() {
    let url = "https://shop.test/big.png";
    let mut big = png(512, 512);
    big.resize(MAX_BYTES + 1, 0);
    let mut host = serve(url, "image/png", big);
    let (entry, data_uri) = load(&mut host, "icon", &Url::parse(url).expect("url"));
    assert_eq!((entry["tooLarge"].as_bool(), data_uri), (Some(true), None));
    assert_eq!(entry["bytes"], MAX_BYTES + 1);

    let mut host = serve(url, "text/html", png(32, 32));
    let (entry, data_uri) = load(&mut host, "icon", &Url::parse(url).expect("url"));
    assert_eq!((entry["tooLarge"].as_bool(), data_uri), (Some(false), None));

    let mut host = MemoryHost::new().grant(Capability::Http);
    let (entry, data_uri) = load(&mut host, "default", &Url::parse(url).expect("url"));
    assert_eq!((entry["status"].as_u64(), data_uri), (Some(404), None));
    assert_eq!(entry["format"], Value::Null);
}
