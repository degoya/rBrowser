use super::*;
use crate::rng::Rng;

fn one(pattern: &str) -> Option<String> {
    sample(pattern, &mut Rng::new(5))
}

/// A tiny matcher for the test's own patterns: each is written as the expected shape.
fn shape(text: &str) -> String {
    text.chars()
        .map(|c| {
            if c.is_ascii_digit() {
                '9'
            } else if c.is_ascii_uppercase() {
                'A'
            } else if c.is_ascii_lowercase() {
                'a'
            } else {
                c
            }
        })
        .collect()
}

#[test]
fn fixed_lengths_and_classes() {
    assert_eq!(one("[0-9]{5}").map(|s| shape(&s)), Some("99999".into()));
    assert_eq!(
        one("\\d{3}-\\d{4}").map(|s| shape(&s)),
        Some("999-9999".into())
    );
    assert_eq!(
        one("[A-Z]{2}[0-9]{2}").map(|s| shape(&s)),
        Some("AA99".into())
    );
    assert_eq!(one("^[a-z]+$").map(|s| shape(&s)), Some("a".into()));
    assert_eq!(one("abc").as_deref(), Some("abc"));
}

#[test]
fn quantifiers_take_their_lower_bound_but_never_nothing_for_a_plus() {
    assert_eq!(one("x{2,4}").as_deref(), Some("xx"));
    assert_eq!(one("x{0,4}").as_deref(), Some("x"));
    assert_eq!(one("x{3,}").as_deref(), Some("xxx"));
    assert_eq!(one("x+").as_deref(), Some("x"));
    assert_eq!(one("x*y").as_deref(), Some("xy"));
    assert_eq!(one("\\+?1").as_deref(), Some("+1"));
    assert_eq!(one("a+?b").as_deref(), Some("ab"));
}

#[test]
fn groups_alternatives_and_escapes() {
    assert_eq!(one("(ab|cd)e").as_deref(), Some("abe"));
    assert_eq!(one("(?:ab){2}").as_deref(), Some("abab"));
    assert_eq!(one("a\\.b").as_deref(), Some("a.b"));
    assert_eq!(
        one("[\\w]{2}").map(|s| s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')),
        Some(true)
    );
    assert_eq!(one("[-+]").map(|s| s == "-" || s == "+"), Some(true));
    assert_eq!(
        one("[^0-9]{3}").map(|s| s.chars().all(|c| !c.is_ascii_digit())),
        Some(true)
    );
}

#[test]
fn unsupported_or_broken_patterns_give_nothing() {
    assert_eq!(one("(?=a)a"), None);
    assert_eq!(one("(a)\\1"), None);
    assert_eq!(one("[a-"), None);
    assert_eq!(one("(ab"), None);
    assert_eq!(one("a{2,1}"), None);
    assert_eq!(one(&"(".repeat(100)), None);
}

#[test]
fn the_output_is_capped() {
    assert_eq!(one("x{100000}"), None);
}
