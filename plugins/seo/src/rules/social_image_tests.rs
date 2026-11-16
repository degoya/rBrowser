use rb_plugin_api::HttpResponse;

use crate::config::Config;
use crate::social::image::tests::{jpeg, png};
use crate::testing::{check_with, serve, short};

const URL: &str = "https://shop.test/";

fn found(image: Option<HttpResponse>, config: &Config) -> Vec<String> {
    let head = r#"<meta property="og:image" content="/share.png">"#;
    let mut host = serve(URL, &format!("<html><head>{head}</head></html>"), &[]);
    if let Some(image) = image {
        host.responses
            .insert("https://shop.test/share.png".to_owned(), image);
    }
    short(
        &check_with(&mut host, config, URL, None, true),
        "social.image_",
    )
    .into_iter()
    .filter(|line| !line.starts_with("social.image_relative"))
    .collect()
}

fn image(kind: &str, body: Vec<u8>) -> Option<HttpResponse> {
    Some(HttpResponse {
        status: 200,
        headers: vec![("content-type".to_owned(), kind.to_owned())],
        body,
    })
}

#[test]
fn a_large_image_of_the_right_ratio_has_no_finding() {
    let config = Config::default();
    assert!(found(image("image/png", png(1200, 630)), &config).is_empty());
    assert!(found(image("image/jpeg", jpeg(2400, 1260)), &config).is_empty());
}

#[test]
fn a_small_image_is_too_small_but_near_the_ratio() {
    assert_eq!(
        found(image("image/png", png(600, 300)), &Config::default()),
        ["social.image_small social.image_small"]
    );
    let config = Config {
        image_min_width: 600,
        image_min_height: 300,
        ..Config::default()
    };
    assert!(found(image("image/png", png(600, 300)), &config).is_empty());
}

#[test]
fn a_square_image_is_off_the_ratio() {
    assert_eq!(
        found(image("image/png", png(1200, 1200)), &Config::default()),
        ["social.image_ratio social.image_ratio"]
    );
}

#[test]
fn a_missing_image_html_and_svg_are_named() {
    let config = Config::default();
    assert_eq!(
        found(None, &config),
        ["social.image_unreachable social.image_unreachable"]
    );
    assert_eq!(
        found(image("text/html", b"<!doctype html>".to_vec()), &config),
        ["social.image_type social.image_type"]
    );
    assert_eq!(
        found(image("image/svg+xml", b"<svg/>".to_vec()), &config),
        ["social.image_type social.image_svg"]
    );
}

#[test]
fn a_later_page_of_a_crawl_fetches_no_image() {
    let head = r#"<meta property="og:image" content="/share.png">"#;
    let mut host = serve(URL, &format!("<html><head>{head}</head></html>"), &[]);
    let _ = check_with(&mut host, &Config::default(), URL, None, false);
    assert!(
        host.fetched
            .iter()
            .all(|request| !request.url.ends_with("share.png"))
    );
}

// RB-080-99: the page asks for the large card, but a chosen platform's rule shows the image small.
fn cards_small(head: &str, image: Vec<u8>, platforms: &[&str]) -> Vec<String> {
    let mut host = serve(URL, &format!("<html><head>{head}</head></html>"), &[]);
    host.responses.insert(
        "https://shop.test/share.png".to_owned(),
        HttpResponse {
            status: 200,
            headers: vec![("content-type".to_owned(), "image/png".to_owned())],
            body: image,
        },
    );
    let config = Config {
        social_platforms: platforms.iter().map(|id| (*id).to_owned()).collect(),
        ..Config::default()
    };
    short(
        &check_with(&mut host, &config, URL, None, true),
        "social.card_small",
    )
}

const LARGE: &str = r#"<meta property="og:image" content="/share.png">
<meta name="twitter:card" content="summary_large_image">"#;

#[test]
fn a_platform_that_shows_the_image_small_against_the_page_s_wish_is_named() {
    let found = cards_small(
        LARGE,
        png(400, 400),
        &["facebook", "linkedin", "whatsapp", "x"],
    );
    assert_eq!(
        found,
        [
            "social.card_small social.card_small_below",
            "social.card_small social.card_small_narrow"
        ]
    );
}

#[test]
fn no_card_finding_without_the_wish_or_with_a_large_enough_image() {
    let head = r#"<meta property="og:image" content="/share.png">"#;
    assert!(cards_small(head, png(400, 400), &["facebook"]).is_empty());
    assert!(cards_small(LARGE, png(1200, 630), &["facebook", "linkedin", "signal"]).is_empty());
    // A platform the settings do not choose is not checked.
    assert!(cards_small(LARGE, png(400, 400), &["x"]).is_empty());
}
