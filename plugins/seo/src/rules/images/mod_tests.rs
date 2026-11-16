use crate::testing::{check, serve, short};

const URL: &str = "https://shop.test/";

fn images(body: &str) -> Vec<String> {
    let mut host = serve(
        URL,
        &format!("<html lang=en><body>{body}</body></html>"),
        &[],
    );
    short(&check(&mut host, URL, None), "images")
}

#[test]
fn a_good_alt_and_a_decorative_empty_one_are_quiet() {
    assert!(images(r#"<img src="bike.jpg" alt="A red bicycle against a wall">"#).is_empty());
    assert!(images(r#"<img src="line.png" alt="" width="20" height="2">"#).is_empty());
    assert!(images(r#"<img src="deco.png" alt="" role="presentation">"#).is_empty());
    assert!(images(r#"<img src="deco.png" alt="" aria-hidden="true">"#).is_empty());
    assert!(images(r#"<a href="/x">Read more <img src="arrow.png" alt=""></a>"#).is_empty());
}

#[test]
fn missing_empty_and_linked_images_are_told_apart() {
    assert_eq!(
        images(r#"<img src="a.jpg">"#),
        ["images.alt_missing images.alt_missing"]
    );
    assert_eq!(
        images(r#"<img src="a.jpg" alt="">"#),
        ["images.alt_empty images.alt_empty"]
    );
    assert_eq!(
        images(r#"<a href="/"><img src="logo.png" alt=""></a>"#),
        ["images.linked_no_name images.linked_no_name"]
    );
    assert_eq!(
        images(r#"<a href="/"><img src="logo.png"></a>"#),
        [
            "images.alt_missing images.alt_missing",
            "images.linked_no_name images.linked_no_name"
        ]
    );
    assert!(images(r#"<a href="/" aria-label="Home"><img src="logo.png" alt=""></a>"#).is_empty());
}

#[test]
fn a_lazy_image_inside_picture_keeps_its_source() {
    let mut host = serve(
        URL,
        r#"<html lang=en><body><picture><source srcset="a.avif"><img data-src="lazy.jpg" loading="lazy"></picture></body></html>"#,
        &[],
    );
    let findings = check(&mut host, URL, None);
    let missing = findings
        .iter()
        .find(|finding| finding.rule == "images.alt_missing")
        .expect("missing");
    assert!(
        missing
            .params
            .contains(&("src".to_owned(), "lazy.jpg".to_owned()))
    );
    assert_eq!(missing.path.as_deref(), Some("html > body > picture > img"));
}

#[test]
fn the_same_alt_on_two_images_and_an_alt_that_repeats_its_caption_are_reported() {
    assert_eq!(
        images(
            r#"<img src="a.jpg" alt="Our team"><img src="b.jpg" alt="Our team"><img src="a.jpg" alt="Our team">"#
        ),
        ["images.alt_duplicate images.alt_duplicate"]
    );
    assert_eq!(
        images(
            r#"<figure><img src="a.jpg" alt="The harbour at dawn"><figcaption>The harbour at dawn</figcaption></figure>"#
        ),
        ["images.alt_redundant images.alt_redundant"]
    );
}
