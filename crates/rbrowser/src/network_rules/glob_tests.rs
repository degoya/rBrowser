use super::*;

#[test]
fn a_star_matches_any_characters_slashes_included() {
    assert!(matches("*/api/user", "http://127.0.0.1:8080/api/user"));
    assert!(matches("*/tracking.js", "https://cdn.test/a/b/tracking.js"));
    assert!(matches("*", ""));
    assert!(matches("https://*.test/*", "https://a.test/x"));
    assert!(!matches(
        "*/api/user",
        "http://127.0.0.1:8080/api/user?id=1"
    ));
    assert!(matches(
        "*/api/user*",
        "http://127.0.0.1:8080/api/user?id=1"
    ));
}

#[test]
fn a_question_mark_matches_exactly_one_character() {
    assert!(matches("http://a.test/?.png", "http://a.test/1.png"));
    assert!(!matches("http://a.test/?.png", "http://a.test/12.png"));
    assert!(!matches("http://a.test/?.png", "http://a.test/.png"));
}

#[test]
fn a_backslash_escapes_the_wildcards() {
    assert!(matches(r"http://a.test/\*", "http://a.test/*"));
    assert!(!matches(r"http://a.test/\*", "http://a.test/x"));
    assert!(matches(r"http://a.test/\?q", "http://a.test/?q"));
    assert!(matches(r"a\\b", r"a\b"));
}

#[test]
fn the_whole_text_must_match() {
    assert!(!matches("api", "http://a.test/api"));
    assert!(!matches("http://a.test/", "http://a.test/x"));
    assert!(matches("http://a.test/x", "http://a.test/x"));
}

#[test]
fn many_stars_stay_fast() {
    let pattern = "*a*a*a*a*a*a*a*a*a*a*b";
    let text = "a".repeat(4000);
    assert!(!matches(pattern, &text));
}
