use super::{MAX_URL_CHARS, Refusal, checked};

#[test]
fn web_addresses_open_normalised() {
    assert_eq!(
        checked("https://dequeuniversity.com/rules/axe/4.10/color-contrast?application=axeAPI")
            .expect("opens")
            .as_str(),
        "https://dequeuniversity.com/rules/axe/4.10/color-contrast?application=axeAPI"
    );
    assert_eq!(
        checked("HTTP://Example.COM/a%20b").expect("opens").as_str(),
        "http://example.com/a%20b"
    );
}

#[test]
fn every_other_scheme_is_refused() {
    for text in [
        "file:///C:/Windows/System32/calc.exe",
        "javascript:alert(1)",
        "rbrowser://settings/ui.theme",
        "ms-settings:privacy",
        "data:text/html,<script>alert(1)</script>",
        "vbscript:msgbox",
        "mailto:someone@example.com",
        "\\\\server\\share\\run.exe",
        "C:\\Windows\\notepad.exe",
    ] {
        let refusal = checked(text).expect_err(text);
        assert!(
            matches!(refusal, Refusal::Scheme | Refusal::Invalid),
            "{text}"
        );
    }
    assert_eq!(checked("file:///etc/passwd"), Err(Refusal::Scheme));
    assert_eq!(checked("javascript:alert(1)"), Err(Refusal::Scheme));
}

#[test]
fn addresses_that_hide_or_break_their_host_are_refused() {
    for text in [
        "",
        "https://",
        "https://user:secret@bank.example/",
        "https://bank.example@evil.example/",
        "https://site.test/ \"&calc",
        "https://site.test/\u{0}",
        "https://site.test/\nnext",
        "  https://site.test/",
        "not a url",
    ] {
        assert_eq!(checked(text), Err(Refusal::Invalid), "{text:?}");
    }
    let long = format!("https://site.test/{}", "a".repeat(MAX_URL_CHARS));
    assert_eq!(checked(&long), Err(Refusal::Invalid));
}

#[test]
fn a_refusal_is_a_sentence_of_the_catalogue() {
    assert_eq!(Refusal::Scheme.key(), "links.error.scheme");
    assert_eq!(Refusal::Invalid.key(), "links.error.invalid");
}
