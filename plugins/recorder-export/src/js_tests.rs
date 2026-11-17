use super::{SECRET_HELPER, Secrets, comment, number, relative_url, secret, string};

#[test]
fn a_string_is_single_quoted_with_every_breaking_character_escaped() {
    assert_eq!(string("plain"), "'plain'");
    assert_eq!(string("it's"), r"'it\'s'");
    assert_eq!(string(r"a\b"), r"'a\\b'");
    assert_eq!(string("two\nlines\r"), r"'two\nlines\r'");
    assert_eq!(string("tab\tnul\0"), r"'tab\tnul\x00'");
    assert_eq!(string("sep\u{2028}\u{2029}"), r"'sep\u2028\u2029'");
    assert_eq!(string("bell\u{7}"), r"'bell\x07'");
    assert_eq!(string("Gr\u{fc}\u{df}e"), "'Gr\u{fc}\u{df}e'");
}

#[test]
fn a_secret_placeholder_becomes_a_checked_environment_variable() {
    let mut secrets = Secrets::default();
    assert_eq!(
        secret("{{secret:password}}", &mut secrets).as_deref(),
        Some("rbSecret('RB_SECRET_PASSWORD')")
    );
    assert_eq!(
        secret("{{secret:one-time_code}}", &mut secrets).as_deref(),
        Some("rbSecret('RB_SECRET_ONE_TIME_CODE')")
    );
    assert_eq!(
        secrets.names(),
        ["RB_SECRET_ONE_TIME_CODE", "RB_SECRET_PASSWORD"]
    );
}

#[test]
fn only_exactly_one_valid_placeholder_is_a_secret() {
    let mut secrets = Secrets::default();
    for text in [
        "{{secret:}}",
        "{{secret:a b}}",
        "{{secret:open",
        "pin-{{secret:pw}}",
        "{{secret:pw}}!",
        "{{secret:a}}{{secret:b}}",
        "alice@example.org",
    ] {
        assert_eq!(secret(text, &mut secrets), None, "{text}");
    }
    assert!(secrets.names().is_empty());
}

/// RB-050-02 review: a missing secret stops the step with its name instead of filling ''.
#[test]
fn the_helper_throws_when_the_variable_is_missing() {
    assert!(SECRET_HELPER.starts_with("function rbSecret(name: string): string {\n"));
    assert!(SECRET_HELPER.contains("throw new Error("));
    assert!(!SECRET_HELPER.contains("?? ''"));
}

#[test]
fn a_comment_is_one_line_without_control_characters() {
    assert_eq!(comment("Log in\nnow */ please"), "Log in now */ please");
    assert_eq!(comment("  spaced\t out  "), "spaced out");
}

#[test]
fn numbers_are_plain_and_finite() {
    assert_eq!(number(2.0), "2");
    assert_eq!(number(2.625), "2.625");
    assert_eq!(number(-12.5), "-12.5");
    assert_eq!(number(f64::NAN), "0");
    assert_eq!(number(f64::INFINITY), "0");
}

#[test]
fn a_url_of_the_base_origin_becomes_a_path() {
    let base = Some("http://127.0.0.1:8000");
    assert_eq!(
        relative_url("http://127.0.0.1:8000/recorder.html?x=1#y", base),
        "/recorder.html?x=1#y"
    );
    assert_eq!(relative_url("http://127.0.0.1:8000", base), "/");
    assert_eq!(
        relative_url("http://127.0.0.1:8000/", Some("http://127.0.0.1:8000/")),
        "/"
    );
    assert_eq!(relative_url("http://127.0.0.1:8000?q", base), "/?q");
}

#[test]
fn another_origin_or_a_base_with_a_path_keeps_the_url() {
    let url = "http://127.0.0.1:8000/a";
    assert_eq!(relative_url(url, Some("http://127.0.0.1:8001")), url);
    assert_eq!(relative_url(url, Some("http://127.0.0.1:800")), url);
    assert_eq!(relative_url(url, Some("http://127.0.0.1:8000/app")), url);
    assert_eq!(relative_url(url, Some("not a url")), url);
    assert_eq!(relative_url(url, None), url);
}
