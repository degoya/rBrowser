use super::*;
use crate::config::Config;

#[test]
fn file_names_are_known() {
    for alt in [
        "IMG_1234.jpg",
        "IMG_1234",
        "DSC01234",
        "hero-final-v2",
        "team_photo_2024",
        "banner.webp",
    ] {
        assert!(is_filename(alt, "/x.jpg"), "{alt}");
    }
    assert!(is_filename("summer-sale", "/img/summer-sale.png"));
    for alt in [
        "A red bicycle",
        "e-commerce",
        "Logo of ACME Inc.",
        "Imgur",
        "Version 2.0 released",
    ] {
        assert!(!is_filename(alt, "/x.jpg"), "{alt}");
    }
}

#[test]
fn placeholders_alone_are_known() {
    let words = Config::default().placeholders;
    assert!(is_placeholder("Image", &words));
    assert!(is_placeholder(" logo. ", &words));
    assert!(is_placeholder("Bild", &words));
    assert!(!is_placeholder("Logo of ACME", &words));
}

#[test]
fn prefixes_that_say_image_are_known() {
    assert_eq!(redundant_prefix("Image of a cat"), Some("image of"));
    assert_eq!(redundant_prefix("Bild von einem Hund"), Some("bild von"));
    assert_eq!(redundant_prefix("Imagery of the coast"), None);
    assert_eq!(redundant_prefix("A cat"), None);
}

#[test]
fn a_long_alt_in_another_language_is_named() {
    assert_eq!(
        foreign_language(
            "Der kleine Hund spielt mit dem roten Ball auf der Wiese hinter dem Haus",
            "en"
        ),
        Some("German")
    );
    assert_eq!(
        foreign_language(
            "Der kleine Hund spielt mit dem roten Ball auf der Wiese hinter dem Haus",
            "de-AT"
        ),
        None
    );
    assert_eq!(
        foreign_language("Roter Hund", "en"),
        None,
        "too short to tell"
    );
    assert_eq!(
        foreign_language(
            "Der kleine Hund spielt mit dem roten Ball auf der Wiese",
            "xx"
        ),
        None
    );
}
