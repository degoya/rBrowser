use rb_plugin_api::Severity;

use super::*;
use crate::cookies::parse_set_cookie;

/// The headers of a page that passes every rule.
fn good() -> Vec<(&'static str, &'static str)> {
    vec![
        (
            "content-security-policy",
            "default-src 'self'; script-src 'self' 'nonce-r4nd'; object-src 'none'; base-uri 'self'; frame-ancestors 'none'",
        ),
        (
            "strict-transport-security",
            "max-age=63072000; includeSubDomains; preload",
        ),
        ("x-frame-options", "DENY"),
        ("x-content-type-options", "nosniff"),
        ("referrer-policy", "strict-origin-when-cross-origin"),
        ("permissions-policy", "camera=(), geolocation=()"),
    ]
}

fn document(url: &str, headers: &[(&str, &str)], cookies: &[&str]) -> Document {
    let pairs: Vec<(String, String)> = headers
        .iter()
        .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
        .collect();
    Document {
        url: url.to_owned(),
        downgrade: None,
        headers: Headers::from_pairs(&pairs),
        cookies: cookies
            .iter()
            .filter_map(|line| parse_set_cookie(line))
            .collect(),
    }
}

fn rules_of(doc: &Document) -> Vec<&'static str> {
    audit(doc, &Settings::default())
        .into_iter()
        .map(|issue| issue.rule)
        .collect()
}

fn without(name: &str) -> Vec<(&'static str, &'static str)> {
    good()
        .into_iter()
        .filter(|(header, _)| *header != name)
        .collect()
}

fn replaced(name: &'static str, value: &'static str) -> Vec<(&'static str, &'static str)> {
    let mut headers = without(name);
    headers.push((name, value));
    headers
}

const HTTPS: &str = "https://example.test/";

#[test]
fn a_good_page_has_no_issue() {
    let doc = document(HTTPS, &good(), &["sid=x; Secure; HttpOnly; SameSite=Lax"]);
    assert_eq!(rules_of(&doc), Vec::<&str>::new());
}

#[test]
fn a_missing_header_is_found_for_each_rule() {
    let cases = [
        ("content-security-policy", rules::CSP_MISSING),
        ("strict-transport-security", rules::HSTS_MISSING),
        ("x-content-type-options", rules::NOSNIFF_MISSING),
        ("referrer-policy", rules::REFERRER_MISSING),
        ("permissions-policy", rules::PERMISSIONS_MISSING),
    ];
    for (header, rule) in cases {
        assert_eq!(
            rules_of(&document(HTTPS, &without(header), &[])),
            [rule],
            "{header}"
        );
    }
}

#[test]
fn frame_protection_comes_from_either_header() {
    let csp_only = without("x-frame-options");
    assert!(rules_of(&document(HTTPS, &csp_only, &[])).is_empty());
    let xfo_only = replaced(
        "content-security-policy",
        "script-src 'self'; object-src 'none'; base-uri 'none'",
    );
    assert!(rules_of(&document(HTTPS, &xfo_only, &[])).is_empty());
    let mut neither = xfo_only;
    neither.retain(|(name, _)| *name != "x-frame-options");
    assert_eq!(
        rules_of(&document(HTTPS, &neither, &[])),
        [rules::FRAME_MISSING]
    );
    let obsolete = replaced("x-frame-options", "ALLOW-FROM https://other.test");
    let mut obsolete: Vec<_> = obsolete
        .into_iter()
        .map(|(name, value)| {
            if name == "content-security-policy" {
                (
                    name,
                    "script-src 'self'; object-src 'none'; base-uri 'none'",
                )
            } else {
                (name, value)
            }
        })
        .collect();
    assert_eq!(
        rules_of(&document(HTTPS, &obsolete, &[])),
        [rules::FRAME_MISSING]
    );
    obsolete.push(("x-frame-options", "SAMEORIGIN"));
    assert_eq!(
        rules_of(&document(HTTPS, &obsolete, &[])),
        [rules::FRAME_MISSING]
    );
}

#[test]
fn csp_findings_name_what_they_found() {
    let doc = document(
        HTTPS,
        &replaced(
            "content-security-policy",
            "script-src 'self' 'unsafe-inline' 'unsafe-eval' https:; frame-ancestors 'self'",
        ),
        &[],
    );
    let issues = audit(&doc, &Settings::default());
    let rules: Vec<&str> = issues.iter().map(|issue| issue.rule).collect();
    assert_eq!(
        rules,
        [
            rules::CSP_UNSAFE,
            rules::CSP_WILDCARD,
            rules::CSP_OBJECT_SRC,
            rules::CSP_BASE_URI
        ]
    );
    assert_eq!(
        issues[0].params,
        [("keywords", "'unsafe-inline', 'unsafe-eval'".to_owned())]
    );
    assert_eq!(issues[1].params, [("sources", "https:".to_owned())]);
}

#[test]
fn a_csp_without_script_rules_is_missing_for_scripts() {
    let doc = document(
        HTTPS,
        &replaced(
            "content-security-policy",
            "object-src 'none'; base-uri 'self'; frame-ancestors 'none'",
        ),
        &[],
    );
    let issues = audit(&doc, &Settings::default());
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].rule, rules::CSP_MISSING);
    assert_eq!(issues[0].variant, Some("script_src"));
}

#[test]
fn hsts_is_held_to_the_minimum_age() {
    let short = document(
        HTTPS,
        &replaced("strict-transport-security", "max-age=600"),
        &[],
    );
    let issues = audit(&short, &Settings::default());
    assert_eq!(issues[0].rule, rules::HSTS_SHORT);
    assert_eq!(
        issues[0].params,
        [
            ("maxAge", "600".to_owned()),
            ("minAge", "31536000".to_owned())
        ]
    );
    let lenient = Settings { hsts_min_age: 600 };
    assert!(audit(&short, &lenient).is_empty());
    let quoted = document(
        HTTPS,
        &replaced(
            "strict-transport-security",
            "includeSubDomains; MAX-AGE=\"31536000\"",
        ),
        &[],
    );
    assert!(rules_of(&quoted).is_empty());
    for broken in ["max-age=", "max-age=-5", "max-age=1e9", "includeSubDomains"] {
        let doc = document(HTTPS, &replaced("strict-transport-security", broken), &[]);
        assert_eq!(rules_of(&doc), [rules::HSTS_MISSING], "{broken}");
    }
    let huge = document(
        HTTPS,
        &replaced(
            "strict-transport-security",
            "max-age=99999999999999999999999999",
        ),
        &[],
    );
    assert!(rules_of(&huge).is_empty());
}

#[test]
fn hsts_is_not_asked_over_http_or_on_localhost() {
    for url in [
        "http://example.test/",
        "http://localhost:3000/",
        "https://localhost/",
        "https://app.localhost:8443/",
        "https://127.0.0.1/",
        "https://[::1]:4430/",
    ] {
        let doc = document(url, &without("strict-transport-security"), &[]);
        assert!(rules_of(&doc).is_empty(), "{url}");
    }
}

#[test]
fn an_unsafe_referrer_policy_is_found_by_its_last_known_token() {
    let unsafe_url = document(
        HTTPS,
        &replaced("referrer-policy", "no-referrer, unsafe-url"),
        &[],
    );
    assert_eq!(rules_of(&unsafe_url), [rules::REFERRER_UNSAFE]);
    let fallback = document(
        HTTPS,
        &replaced("referrer-policy", "unsafe-url, no-referrer, made-up"),
        &[],
    );
    assert!(rules_of(&fallback).is_empty());
    let unknown = document(HTTPS, &replaced("referrer-policy", "made-up"), &[]);
    assert_eq!(rules_of(&unknown), [rules::REFERRER_MISSING]);
}

#[test]
fn nosniff_must_be_its_value() {
    let wrong = document(HTTPS, &replaced("x-content-type-options", "sniff"), &[]);
    assert_eq!(rules_of(&wrong), [rules::NOSNIFF_MISSING]);
    let upper = document(HTTPS, &replaced("x-content-type-options", " NoSniff "), &[]);
    assert!(rules_of(&upper).is_empty());
}

#[test]
fn a_wildcard_origin_with_credentials_is_an_error() {
    let mut headers = good();
    headers.push(("access-control-allow-origin", "*"));
    assert!(rules_of(&document(HTTPS, &headers, &[])).is_empty());
    headers.push(("access-control-allow-credentials", "TRUE"));
    let issues = audit(&document(HTTPS, &headers, &[]), &Settings::default());
    assert_eq!(issues[0].rule, rules::CORS_CREDENTIALS);
    assert_eq!(rules::severity_of(issues[0].rule), Severity::Error);
}

#[test]
fn each_cookie_is_one_finding_with_what_it_lacks() {
    let doc = document(
        HTTPS,
        &good(),
        &[
            "sid=TOPSECRET; Path=/; HttpOnly",
            "__Host-pref=TOPSECRET; Secure; HttpOnly; SameSite=Lax; Path=/sub",
        ],
    );
    let issues = audit(&doc, &Settings::default());
    let rules: Vec<&str> = issues.iter().map(|issue| issue.rule).collect();
    assert_eq!(rules, [rules::COOKIE_FLAGS, rules::COOKIE_PREFIX]);
    assert_eq!(
        issues[0].params,
        [
            ("cookie", "sid".to_owned()),
            ("missing", "Secure, SameSite".to_owned())
        ]
    );
    assert_eq!(issues[0].path.as_deref(), Some("cookie:sid"));
    assert_eq!(issues[1].path.as_deref(), Some("cookie:__Host-pref"));
    assert!(!format!("{issues:?}").contains("TOPSECRET"));
}

#[test]
fn a_cookie_over_http_is_not_asked_for_secure() {
    let doc = document(
        "http://localhost:3000/",
        &good(),
        &["sid=x; HttpOnly; SameSite=Lax"],
    );
    assert!(rules_of(&doc).is_empty());
}

#[test]
fn findings_of_the_csp_carry_its_value() {
    let doc = document(HTTPS, &without("content-security-policy"), &[]);
    let issues = audit(&doc, &Settings::default());
    assert_eq!(issues[0].data["header"], "content-security-policy");
    let doc = document(
        HTTPS,
        &replaced(
            "content-security-policy",
            "script-src 'unsafe-eval'; object-src 'none'; base-uri 'none'",
        ),
        &[],
    );
    let issues = audit(&doc, &Settings::default());
    assert_eq!(
        issues[0].data["value"],
        "script-src 'unsafe-eval'; object-src 'none'; base-uri 'none'"
    );
}

#[test]
fn a_redirect_from_https_to_http_is_reported_even_when_the_end_is_https() {
    let mut doc = document(HTTPS, &good(), &[]);
    doc.downgrade = Some("http://example.test/plain".to_owned());
    let issues = audit(&doc, &Settings::default());
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].rule, rules::REDIRECT_DOWNGRADE);
    assert_eq!(
        issues[0].params,
        [("url", "http://example.test/plain".to_owned())]
    );
    let mut plain_end = document("http://example.test/plain", &good(), &[]);
    plain_end.downgrade = Some("http://example.test/plain".to_owned());
    assert_eq!(rules_of(&plain_end), [rules::REDIRECT_DOWNGRADE]);
    let mut local = document("http://localhost:3000/", &good(), &[]);
    local.downgrade = Some("http://localhost:3000/".to_owned());
    assert!(rules_of(&local).is_empty());
}

#[test]
fn the_rules_sharing_a_lighthouse_id_never_meet_on_one_page() {
    // The findings run merges by shared id, URL and path: two rules with one shared id must
    // never both be found on one page, or one would swallow the other.
    let pages: Vec<Vec<(&str, &str)>> = vec![
        without("content-security-policy"),
        replaced("content-security-policy", "object-src 'none'"),
        replaced(
            "content-security-policy",
            "script-src 'unsafe-inline' 'unsafe-eval'",
        ),
        replaced(
            "content-security-policy",
            "default-src *; script-src 'unsafe-inline'",
        ),
        without("strict-transport-security"),
        replaced("strict-transport-security", "max-age=5"),
        replaced("strict-transport-security", "max-age=nope"),
    ];
    for headers in pages {
        let found = rules_of(&document(HTTPS, &headers, &[]));
        for shared in ["csp-xss", "has-hsts", "clickjacking-mitigation"] {
            let claiming = found
                .iter()
                .filter(|rule| rules::find(rule).and_then(|rule| rule.shared_id) == Some(shared))
                .count();
            assert!(claiming <= 1, "{shared}: {found:?}");
        }
    }
}
