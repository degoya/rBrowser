use super::*;

fn csp(headers: &[&str]) -> Csp {
    Csp::parse(headers.iter().copied())
}

#[test]
fn no_header_is_an_empty_policy_set() {
    let csp = csp(&[]);
    assert!(csp.is_empty());
    assert!(!csp.restricts_scripts());
}

#[test]
fn unsafe_inline_in_script_src_is_found() {
    let csp = csp(&["script-src 'self' 'unsafe-inline'; object-src 'none'; base-uri 'self'"]);
    assert!(csp.restricts_scripts());
    assert_eq!(csp.unsafe_keywords(), ["'unsafe-inline'"]);
    assert!(csp.restricts_objects());
    assert!(csp.has_base_uri());
}

#[test]
fn default_src_stands_in_for_script_src_and_keywords_ignore_case() {
    let csp = csp(&["DEFAULT-SRC 'SELF' 'Unsafe-Eval'"]);
    assert!(csp.restricts_scripts());
    assert_eq!(csp.unsafe_keywords(), ["'unsafe-eval'"]);
    assert!(csp.restricts_objects());
    assert!(!csp.has_base_uri());
}

#[test]
fn a_nonce_a_hash_or_strict_dynamic_disables_unsafe_inline() {
    for policy in [
        "script-src 'unsafe-inline' 'nonce-abc'",
        "script-src 'unsafe-inline' 'sha256-AAAA'",
        "script-src 'unsafe-inline' 'strict-dynamic'",
    ] {
        assert!(
            csp(&[policy]).unsafe_keywords().is_empty(),
            "{policy} keeps inline scripts out"
        );
    }
}

#[test]
fn a_second_policy_that_forbids_it_wins() {
    let csp = csp(&[
        "script-src 'unsafe-inline' 'unsafe-eval'",
        "script-src 'self'",
    ]);
    assert!(csp.unsafe_keywords().is_empty());
    let joined = Csp::parse(["script-src 'unsafe-inline', script-src 'self' 'unsafe-inline'"]);
    assert_eq!(joined.unsafe_keywords(), ["'unsafe-inline'"]);
}

#[test]
fn a_policy_without_script_rules_does_not_restrict_scripts() {
    let csp = csp(&["frame-ancestors 'none'; img-src *"]);
    assert!(!csp.is_empty());
    assert!(!csp.restricts_scripts());
    assert!(csp.has_frame_ancestors());
    assert!(csp.unsafe_keywords().is_empty());
}

#[test]
fn wildcards_in_the_script_sources_are_found() {
    assert_eq!(
        csp(&["script-src 'self' * https: data:"]).wildcards(),
        ["*", "https:", "data:"]
    );
    assert_eq!(
        csp(&["script-src https://* http://cdn.example.test"]).wildcards(),
        ["https://*"]
    );
    assert!(
        csp(&["script-src 'self' https://*.example.test"])
            .wildcards()
            .is_empty()
    );
    assert!(
        csp(&["script-src 'nonce-x' 'strict-dynamic' https:"])
            .wildcards()
            .is_empty()
    );
}

#[test]
fn the_first_of_a_repeated_directive_counts() {
    let csp = csp(&["script-src 'self'; script-src 'unsafe-inline'"]);
    assert!(csp.unsafe_keywords().is_empty());
}

#[test]
fn hostile_policies_are_bounded_and_never_panic() {
    let long_token = format!("script-src {}", "a".repeat(100_000));
    let many_directives = (0..10_000)
        .map(|n| format!("x{n}-src 'self'"))
        .collect::<Vec<_>>()
        .join(";");
    let many_policies = vec!["script-src 'unsafe-inline'"; 1000].join(",");
    let many_sources = format!("script-src {}", vec!["'self'"; 100_000].join(" "));
    for text in [
        long_token.as_str(),
        many_directives.as_str(),
        many_policies.as_str(),
        many_sources.as_str(),
        ";;;,,, ;",
        "\u{0}\u{1} script-src",
        "script-src '\u{e9}\u{1F600}' 'unsafe-inline",
        "",
    ] {
        let csp = Csp::parse([text]);
        assert!(csp.policies().len() <= MAX_POLICIES);
        for policy in csp.policies() {
            assert!(policy.directive_count() <= MAX_DIRECTIVES);
        }
        let _ = (
            csp.unsafe_keywords(),
            csp.wildcards(),
            csp.restricts_objects(),
        );
    }
}

#[test]
fn a_directive_name_must_be_a_name() {
    let csp = csp(&["script\u{0}src 'unsafe-inline'; scr@pt-src 'unsafe-inline'"]);
    assert!(!csp.restricts_scripts());
}
