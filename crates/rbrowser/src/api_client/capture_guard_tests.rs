use super::*;

fn own(name: &str) -> Variable {
    Variable {
        name: name.into(),
        value: Some("value-123456".into()),
    }
}

#[test]
fn only_the_captures_own_variables_pass() {
    let mine = [own("a-test-token")];
    assert!(!foreign("Bearer {{a-test-token}}", &mine));
    assert!(!foreign("no template", &mine));
    for text in [
        "{{b-test-token}}",
        "{{$secret x}}",
        "{{ a-test-token }}",
        "{{{a-test-token}}}",
        "{{a-test-token}} {{",
        "a {{ lone",
    ] {
        assert!(foreign(text, &mine), "{text}");
    }
}

#[test]
fn random_segments_are_told_from_words_and_ids() {
    for segment in [
        "aB3dE5fG7hI9jK1lM3nO",
        "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b",
        "550e8400-e29b-41d4-a716-446655440000",
        "k3j9x0q2m8z7w1v5b6n4",
        // A bot token's shape, split so no secret scanner reads the fixture as a live token.
        concat!("bot123456789", ":AAHdqTcvCH1vGWJxfSeofSAs0K5PALDsaw"),
    ] {
        assert!(looks_random(segment), "{segment}");
    }
    for segment in [
        "this-is-a-long-article-slug-2024",
        "introducing-rbrowser-2026-the-dev-browser",
        "annual-report-q3-2025-final-version",
        "12345678901234567890",
        "short1A",
        "%7B%7Bsecret%7D%7Dabcdef",
    ] {
        assert!(!looks_random(segment), "{segment}");
    }
}

#[test]
fn a_secret_name_or_link_word_names_the_next_segment() {
    assert!(names_secret("reset") && names_secret("Invite") && names_secret("token"));
    assert!(!names_secret("users") && !names_secret("v1"));
    assert!(is_plain_word("refresh") && !is_plain_word("x7k2m9"));
}
