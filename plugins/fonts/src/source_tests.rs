use super::*;
use crate::page::Src;

const PAGE: &str = "https://www.example.co.uk/shop/";

fn face(src: &[(&str, &str, bool)], sheet: &str) -> Face {
    Face {
        family: "Inter".to_owned(),
        src: src
            .iter()
            .map(|(url, format, local)| Src {
                url: (*url).to_owned(),
                format: (*format).to_owned(),
                local: *local,
            })
            .collect(),
        sheet: sheet.to_owned(),
        ..Face::default()
    }
}

#[test]
fn hosts_are_read_without_user_info_and_port() {
    assert_eq!(
        host_of("https://u:p@Fonts.Example.com:8443/a.woff2").as_deref(),
        Some("fonts.example.com")
    );
    assert_eq!(host_of("http://[::1]:8080/x").as_deref(), Some("[::1]"));
    assert_eq!(host_of("data:font/woff2;base64,AA"), None);
    assert_eq!(host_of("ftp://x/y"), None);
}

#[test]
fn sites_take_the_registrable_domain() {
    assert_eq!(site_of("cdn.static.example.com"), "example.com");
    assert_eq!(site_of("www.example.co.uk"), "example.co.uk");
    assert_eq!(site_of("localhost"), "localhost");
    assert_eq!(site_of("127.0.0.1"), "127.0.0.1");
}

#[test]
fn providers_by_host() {
    let google = classify("https://fonts.gstatic.com/s/roboto/a.woff2", PAGE);
    assert_eq!(google, (Provider::Google, "fonts.gstatic.com".to_owned()));
    assert_eq!(
        classify("https://use.typekit.net/abc.css", PAGE).0,
        Provider::Adobe
    );
    assert_eq!(
        classify("https://static.example.co.uk/f.woff2", PAGE).0,
        Provider::Own
    );
    assert_eq!(
        classify("https://cdn.jsdelivr.net/f.woff2", PAGE).0,
        Provider::External
    );
    assert_eq!(
        classify("data:font/woff2;base64,AA", PAGE).0,
        Provider::Inline
    );
    assert!(Provider::Google.third_party() && Provider::External.third_party());
    assert!(!Provider::Own.third_party() && !Provider::System.third_party());
    assert_eq!(
        provider_name(Provider::Adobe, "use.typekit.net"),
        "Adobe Fonts"
    );
    assert_eq!(provider_name(Provider::External, "cdn.x"), "cdn.x");
}

#[test]
fn a_face_by_its_file_its_sheet_or_local() {
    let own = face(
        &[
            ("local", "", true),
            ("https://www.example.co.uk/i.woff2", "woff2", false),
        ],
        "inline",
    );
    assert_eq!(of_face(&own, PAGE).0, Provider::Own);
    let installed = face(&[("Arial", "", true)], "inline");
    assert_eq!(of_face(&installed, PAGE).0, Provider::System);
    let google = face(&[], "https://fonts.googleapis.com/css2?family=Roboto");
    assert_eq!(of_face(&google, PAGE).0, Provider::Google);
    assert_eq!(of_face(&face(&[], ""), PAGE).0, Provider::Unknown);
}

#[test]
fn formats_are_normalised_and_distinct() {
    let face = face(
        &[
            ("a.woff2", "woff2-variations", false),
            ("a.woff2", "woff2", false),
            ("a.ttf", "TrueType", false),
            ("x", "", false),
            ("Arial", "", true),
        ],
        "",
    );
    assert_eq!(formats(&face), ["woff2", "truetype", "unknown"]);
    assert_eq!(urls(&face).count(), 4);
}
