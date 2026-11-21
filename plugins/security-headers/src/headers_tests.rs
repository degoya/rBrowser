use super::*;

fn pairs(list: &[(&str, &str)]) -> Vec<(String, String)> {
    list.iter()
        .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
        .collect()
}

#[test]
fn names_are_read_case_insensitively_and_repeats_are_joined() {
    let headers = Headers::from_pairs(&pairs(&[
        ("Content-Security-Policy", "default-src 'self'"),
        ("X-Frame-Options", "DENY"),
        ("content-security-policy", "object-src 'none'"),
    ]));
    assert_eq!(headers.get("x-frame-options").as_deref(), Some("DENY"));
    assert_eq!(
        headers.all("content-security-policy").collect::<Vec<_>>(),
        ["default-src 'self'", "object-src 'none'"]
    );
    assert_eq!(
        headers.get("content-security-policy").as_deref(),
        Some("default-src 'self', object-src 'none'")
    );
    assert!(headers.has("CONTENT-SECURITY-POLICY"));
    assert!(!headers.has("strict-transport-security"));
}

#[test]
fn hostile_names_are_dropped_and_values_are_bounded() {
    let long = "a".repeat(MAX_VALUE_CHARS + 50);
    let headers = Headers::from_pairs(&pairs(&[
        ("", "empty name"),
        ("bad name", "space"),
        ("x\u{0}y", "nul"),
        ("referrer-policy", "no-referrer\r\nInjected: yes"),
        ("permissions-policy", long.as_str()),
    ]));
    assert_eq!(headers.len(), 2);
    assert_eq!(
        headers.get("referrer-policy").as_deref(),
        Some("no-referrerInjected: yes")
    );
    assert_eq!(
        headers
            .get("permissions-policy")
            .map(|value| value.chars().count()),
        Some(MAX_VALUE_CHARS)
    );
}

#[test]
fn at_most_the_bound_of_headers_is_kept() {
    let many: Vec<(String, String)> = (0..MAX_HEADERS + 40)
        .map(|n| (format!("x-h{n}"), "v".to_owned()))
        .collect();
    assert_eq!(Headers::from_pairs(&many).len(), MAX_HEADERS);
}

#[test]
fn a_multibyte_value_is_cut_on_a_character() {
    let value = "\u{e9}".repeat(MAX_VALUE_CHARS + 1);
    let headers = Headers::from_pairs(&pairs(&[("x-text", value.as_str())]));
    assert_eq!(
        headers.get("x-text").map(|text| text.chars().count()),
        Some(MAX_VALUE_CHARS)
    );
}

#[test]
fn shown_texts_are_short_and_without_controls() {
    assert_eq!(shown("a\u{7}b\nc", 10), "abc");
    assert_eq!(shown(&"x".repeat(300), 5), "xxxxx");
}
