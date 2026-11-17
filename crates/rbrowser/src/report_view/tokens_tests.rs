use std::path::PathBuf;

use super::*;

fn entry(name: &str) -> Entry {
    Entry {
        root: PathBuf::from("/project"),
        plugin: "lighthouse".to_owned(),
        rel: format!("20261112-100000/{name}"),
    }
}

#[test]
fn an_issued_address_resolves_to_its_report() {
    let mut tokens = Tokens::default();
    let url = tokens.issue(entry("mobile.html"));
    assert!(url.starts_with(REPORT_ORIGIN), "{url}");
    assert!(url.ends_with("/mobile.html"), "{url}");
    assert!(is_report_url(&url));
    assert_eq!(tokens.resolve(&url), Some(&entry("mobile.html")));
}

#[test]
fn an_address_with_another_token_or_a_query_resolves_to_nothing() {
    let mut tokens = Tokens::default();
    let url = tokens.issue(entry("mobile.html"));
    let (token, _) = parse_url(&url).expect("an address");
    assert_eq!(token.len(), TOKEN_HEX);
    let forged = url.replace(&token, &"0".repeat(TOKEN_HEX));
    assert_eq!(tokens.resolve(&forged), None);
    assert_eq!(tokens.resolve(&format!("{url}?x=1")), None);
    assert_eq!(
        tokens.resolve(&format!("{url}#top")),
        Some(&entry("mobile.html"))
    );
    assert_eq!(tokens.resolve("rbrowser://ui/index.html"), None);
    assert_eq!(
        tokens.resolve(&format!("{REPORT_ORIGIN}{token}/../x.html")),
        None
    );
    assert_eq!(tokens.resolve(&format!("{REPORT_ORIGIN}{token}")), None);
}

#[test]
fn two_issues_get_two_tokens() {
    let mut tokens = Tokens::default();
    let first = tokens.issue(entry("mobile.html"));
    let second = tokens.issue(entry("mobile.html"));
    assert_ne!(first, second);
}

#[test]
fn a_revoked_address_resolves_to_nothing() {
    let mut tokens = Tokens::default();
    let url = tokens.issue(entry("mobile.html"));
    tokens.revoke(&url);
    assert_eq!(tokens.resolve(&url), None);
}

#[test]
fn the_oldest_token_goes_past_the_limit() {
    let mut tokens = Tokens::default();
    let first = tokens.issue(entry("first.html"));
    for n in 0..MAX_TOKENS {
        tokens.issue(entry(&format!("r{n}.html")));
    }
    assert_eq!(tokens.resolve(&first), None);
    assert_eq!(tokens.len(), MAX_TOKENS);
}

#[test]
fn the_scheme_and_host_compare_without_case() {
    assert!(is_report_url("RBROWSER://Report/abc/x.html"));
    assert!(!is_report_url("rbrowser://ui/settings.html"));
    assert!(!is_report_url("https://report/"));
}

#[test]
fn the_policy_allows_no_network_frame_form_or_base() {
    for directive in [
        "default-src 'none'",
        "connect-src 'none'",
        "frame-src 'none'",
        "form-action 'none'",
        "base-uri 'none'",
        "object-src 'none'",
        "worker-src 'none'",
    ] {
        assert!(CSP.contains(directive), "{directive}");
    }
    assert!(!CSP.contains("http"), "no host is allowed");
    assert!(!CSP.contains("unsafe-eval"));
}

#[test]
fn the_title_names_the_run_and_the_file() {
    assert_eq!(entry("mobile.html").name(), "20261112-100000/mobile.html");
}
