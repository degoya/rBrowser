use super::*;

fn with_headers(headers: &[(&str, &str)]) -> HttpRequest {
    HttpRequest {
        method: "GET".to_owned(),
        url: "https://site.test/".to_owned(),
        headers: headers
            .iter()
            .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
            .collect(),
        body: Vec::new(),
    }
}

#[test]
fn only_accept_and_accept_language_go_out() {
    let request = with_headers(&[
        ("Accept", "application/xml"),
        ("accept-language", "en"),
        ("Cookie", "secret=1"),
        ("Authorization", "Bearer x"),
        ("X-Other", "1"),
    ]);
    assert_eq!(
        outgoing(&request),
        [
            ("Accept".to_owned(), "application/xml".to_owned()),
            ("accept-language".to_owned(), "en".to_owned()),
        ]
    );
}

#[test]
fn a_long_value_or_a_control_character_stays_home() {
    let long = "a".repeat(1025);
    let request = with_headers(&[("Accept", long.as_str()), ("Accept-Language", "en\r\nX: y")]);
    assert!(outgoing(&request).is_empty());
}
