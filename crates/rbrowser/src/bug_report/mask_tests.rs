// The privacy condition of RB-093-04: every masking rule with hostile input. No test registers a
// process-wide secret (`rb_diag::register_secret`): the known values are the masker's own.

use super::*;

/// The three test secrets of the acceptance: a token in the query, an `Authorization` value and
/// a cookie value.
const QUERY_TOKEN: &str = "q-token-4711";
const AUTH_VALUE: &str = "auth-value-4712";
const COOKIE_VALUE: &str = "cookie-value-4713";

fn masker() -> Masker {
    Masker::new([COOKIE_VALUE])
}

fn clean(text: &str) -> bool {
    ![QUERY_TOKEN, AUTH_VALUE, COOKIE_VALUE]
        .iter()
        .any(|secret| text.contains(secret))
}

#[test]
fn every_query_value_of_a_url_is_masked() {
    assert_eq!(
        mask_url("https://example.test/api?token=q-token-4711&page=2"),
        "https://example.test/api?token=***&page=***"
    );
    assert_eq!(
        mask_url("/relative/path?id=7"),
        "/relative/path?id=***",
        "a relative address too"
    );
    assert_eq!(
        mask_url("https://example.test/?q-token-4711"),
        "https://example.test/?***"
    );
    assert_eq!(
        mask_url("https://example.test/?a=&b"),
        "https://example.test/?a=&***"
    );
    assert_eq!(
        mask_url("https://example.test/plain"),
        "https://example.test/plain"
    );
}

#[test]
fn the_fragment_and_the_user_info_go() {
    assert_eq!(
        mask_url("https://example.test/cb#access_token=q-token-4711"),
        "https://example.test/cb#***"
    );
    assert_eq!(
        mask_url("https://user:q-token-4711@example.test/a?b=c"),
        "https://***@example.test/a?b=***"
    );
    assert_eq!(mask_url("https://example.test/#"), "https://example.test/#");
}

#[test]
fn urls_inside_free_text_are_masked_wherever_they_stand() {
    let masker = masker();
    for text in [
        "GET https://example.test/api?token=q-token-4711 401 (Unauthorized)",
        "fetch(\"/api/items?key=q-token-4711\")",
        "{\"url\":\"https://example.test/x?t=q-token-4711\"}",
        "url=https://example.test/x?t=q-token-4711;next",
        "<img src='https://example.test/p.png?sig=q-token-4711'>",
        "HTTPS://EXAMPLE.TEST/?TOKEN=q-token-4711",
        "ws://example.test/socket?auth=q-token-4711",
        "https://\u{4f8b}\u{3048}.test/?q=q-token-4711 \u{1f41b}",
    ] {
        let masked = masker.text(text);
        assert!(clean(&masked), "{text} -> {masked}");
    }
}

#[test]
fn credential_headers_are_masked_in_any_form() {
    let masker = masker();
    for text in [
        "Authorization: Bearer auth-value-4712",
        "authorization: Basic auth-value-4712",
        "AUTHORIZATION=auth-value-4712",
        "request failed {\"Authorization\":\"Basic auth-value-4712\",\"Accept\":\"*/*\"}",
        "headers: { authorization: 'Token auth-value-4712' }",
        "Proxy-Authorization: Digest auth-value-4712",
        "Cookie: sid=cookie-value-4713; theme=dark",
        "set-cookie: sid=cookie-value-4713; Path=/; HttpOnly",
        "x {\"cookie\": \"a=b; sid=cookie-value-4713\"}",
    ] {
        let masked = masker.text(text);
        assert!(clean(&masked), "{text} -> {masked}");
    }
    assert_eq!(
        masker.text("headers {\"Authorization\":\"Basic auth-value-4712\",\"Accept\":\"*/*\"}"),
        "headers {\"Authorization\":\"***\",\"Accept\":\"*/*\"}",
        "only the value goes, the rest of the line stays"
    );
}

#[test]
fn a_known_cookie_value_is_masked_wherever_the_page_logs_it() {
    let masker = masker();
    let masked = masker.text("document.cookie = sid=cookie-value-4713; theme=dark");
    assert!(clean(&masked), "{masked}");
    let masked = masker.text("the session cookie-value-4713 expired");
    assert!(clean(&masked), "{masked}");
    // Too short to be told from ordinary text: never registered.
    assert_eq!(Masker::new(["ab"]).text("tab"), "tab");
}

#[test]
fn text_without_secrets_stays_as_it_is() {
    let masker = masker();
    for text in [
        "TypeError: Cannot read properties of undefined (reading 'x')",
        "at render (https://example.test/app.js:12:5)",
        "Why? Because.",
        "",
    ] {
        assert_eq!(masker.text(text), text);
    }
}

#[test]
fn masking_twice_equals_masking_once() {
    let masker = masker();
    for text in [
        "Authorization: Bearer auth-value-4712",
        "https://u:p@example.test/a?b=c#d",
        "sid=cookie-value-4713",
    ] {
        let once = masker.text(text);
        assert_eq!(masker.text(&once), once, "{text}");
    }
}

#[test]
fn hostile_shapes_do_not_break_the_masker() {
    let masker = masker();
    let long = format!("https://example.test/?{}", "a=b&".repeat(10_000));
    for text in [
        "?",
        "??",
        "?=&=&",
        "=?",
        "://",
        "@://@",
        "#",
        "https://",
        "https://@",
        "?#?#",
        "authorization",
        "authorization:",
        "cookie=\"",
        "\u{0}\u{1b}[31m?x=1",
        long.as_str(),
    ] {
        let _ = masker.text(text);
        let _ = mask_url(text);
    }
    assert!(!masker.text(&long).contains("a=b"));
}

// Security review w27: a token in the path (a reset link, an invitation, a signed download) is
// masked like a query value; words, slugs, numeric ids and short asset hashes stay readable.
#[test]
fn token_shaped_path_segments_are_masked() {
    for (url, masked) in [
        (
            "https://example.test/reset/Zx8kQ2mN4pR7tV9wB3cF6hJ1",
            "https://example.test/reset/***",
        ),
        (
            "https://example.test/invite/550e8400-e29b-41d4-a716-446655440000",
            "https://example.test/invite/***",
        ),
        (
            "https://example.test/f/9f86d081884c7d659a2feaa0c55ad015a3bf4f1b/x.pdf",
            "https://example.test/f/***/x.pdf",
        ),
        (
            "/verify/aGVsbG8tdG9rZW4tdmFsdWU1.c2lnbmF0dXJl",
            "/verify/***",
        ),
        (
            "https://example.test/t/aB3-dE5_fG7-hJ9_kL2-mN4_pQ6",
            "https://example.test/t/***",
        ),
        (
            "https://example.test/u/123456789012345678901234/x?a=b",
            "https://example.test/u/***/x?a=***",
        ),
        (
            "https://example.test/k/QWxhZGRpbjpvcGVuIHNlc2FtZQ:12:5",
            "https://example.test/k/***:12:5",
        ),
    ] {
        assert_eq!(mask_url(url), masked, "{url}");
    }
}

#[test]
fn readable_path_segments_stay() {
    for url in [
        "https://example.test/blog/2024/01/my-first-post-about-rust",
        "https://example.test/users/12345/orders/987654321",
        "https://example.test/assets/index-BvK3x9aQ.js:12:5",
        "https://example.test/docs/getting-started/installation",
        "https://example.test/products/SuperComfortableOfficeChair",
        "https://example.test/en-US/docs/Web/API/XMLHttpRequest",
        "https://example.test/download/release-2024-11-20.tar.gz",
        "https://cdn-9f86d081884c7d659a2f.example.test/app.js",
        "https://example.test/%E4%BE%8B%E3%81%88/%E4%BE%8B%E3%81%88%E4%BE%8B%E3%81%88",
    ] {
        assert_eq!(mask_url(url), url);
    }
}

#[test]
fn a_path_token_in_free_text_is_masked_with_or_without_a_scheme() {
    let masker = masker();
    for (text, masked) in [
        (
            "reset link: https://example.test/reset/Zx8kQ2mN4pR7tV9wB3cF6hJ1 sent",
            "reset link: https://example.test/reset/*** sent",
        ),
        (
            "GET /invite/550e8400-e29b-41d4-a716-446655440000 404",
            "GET /invite/*** 404",
        ),
        (
            "fetch('example.test/dl/9f86d081884c7d659a2feaa0c55ad015')",
            "fetch('example.test/dl/***')",
        ),
    ] {
        let once = masker.text(text);
        assert_eq!(once, masked, "{text}");
        assert_eq!(
            masker.text(&once),
            once,
            "masking twice equals masking once"
        );
    }
    assert_eq!(masker.text("read and/or write"), "read and/or write");
}

#[test]
fn the_credential_names_lie_inside_the_one_list() {
    // RB-094-10: the report masks every query value anyway; its header names are the one list's.
    for name in CREDENTIAL_NAMES {
        assert!(
            rb_util::mask::sensitive_header(name),
            "{name} is missing from rb_util::mask"
        );
    }
}
