use std::collections::BTreeSet;

use serde_json::json;

use super::*;

#[test]
fn every_rule_is_described_once_with_its_key_and_help() {
    let described = describe();
    assert_eq!(described.len(), RULES.len());
    let ids: BTreeSet<&str> = described.iter().map(|rule| rule.id.as_str()).collect();
    assert_eq!(ids.len(), RULES.len());
    for rule in &described {
        assert_eq!(
            rule.title,
            format!("plugin.security-headers.rules.{}", rule.id)
        );
        assert!(
            rule.help_url
                .as_deref()
                .is_some_and(|url| url.starts_with("https://developer.mozilla.org/"))
        );
    }
}

#[test]
fn the_lighthouse_audits_are_shared() {
    let shared = |id: &str| find(id).and_then(|rule| rule.shared_id);
    assert_eq!(shared(CSP_MISSING), Some("csp-xss"));
    assert_eq!(shared(CSP_UNSAFE), Some("csp-xss"));
    assert_eq!(shared(HSTS_MISSING), Some("has-hsts"));
    assert_eq!(shared(HSTS_SHORT), Some("has-hsts"));
    assert_eq!(shared(FRAME_MISSING), Some("clickjacking-mitigation"));
    assert_eq!(shared(COOKIE_FLAGS), None);
}

fn scope<'a>(levels: &'a RuleLevels, enabled: &'a [String]) -> Scope<'a> {
    Scope {
        enabled,
        levels,
        url: "https://example.test/",
        environment: Some("staging"),
    }
}

#[test]
fn a_finding_names_the_url_and_environment_but_no_view() {
    let levels = RuleLevels::default();
    let issue = Issue::new(HSTS_SHORT)
        .param("maxAge", "60")
        .param("minAge", "31536000");
    let finding = finding(&issue, &scope(&levels, &[])).expect("finding");
    assert_eq!(finding.view, None);
    assert_eq!(finding.url.as_deref(), Some("https://example.test/"));
    assert_eq!(finding.severity, Severity::Warning);
    assert_eq!(
        finding.message,
        "plugin.security-headers.findings.hsts.short"
    );
    assert!(
        finding
            .params
            .contains(&("environment".to_owned(), "staging".to_owned()))
    );
    let data: Value = serde_json::from_str(finding.data.as_deref().expect("data")).expect("json");
    assert_eq!(data["environment"], "staging");
}

#[test]
fn rules_not_asked_for_or_off_give_no_finding() {
    let levels = RuleLevels::from_value(&json!({ "csp.missing": "off" }));
    let enabled = vec![NOSNIFF_MISSING.to_owned()];
    assert!(finding(&Issue::new(CSP_MISSING), &scope(&levels, &[])).is_none());
    assert!(finding(&Issue::new(HSTS_MISSING), &scope(&levels, &enabled)).is_none());
    assert!(finding(&Issue::new(NOSNIFF_MISSING), &scope(&levels, &enabled)).is_some());
}

#[test]
fn parameters_are_cut_and_without_controls() {
    let levels = RuleLevels::default();
    let issue = Issue::new(CSP_WILDCARD).param("sources", format!("a\u{1b}b{}", "x".repeat(500)));
    let finding = finding(&issue, &scope(&levels, &[])).expect("finding");
    let (_, value) = &finding.params[0];
    assert_eq!(value.chars().count(), MAX_PARAM_CHARS);
    assert!(value.starts_with("abx"));
}

#[test]
fn the_answer_lists_a_finding_with_its_details() {
    let levels = RuleLevels::default();
    let mut issue = Issue::new(COOKIE_FLAGS).param("cookie", "sid");
    issue.path = Some("cookie:sid".to_owned());
    issue.data = json!({ "cookie": "sid" });
    let finding = finding(&issue, &scope(&levels, &[])).expect("finding");
    let shown = finding_json(&finding);
    assert_eq!(shown["rule"], "cookie.flags");
    assert_eq!(shown["severity"], "warning");
    assert_eq!(shown["params"]["cookie"], "sid");
    assert_eq!(shown["path"], "cookie:sid");
    assert_eq!(shown["data"]["cookie"], "sid");
}

#[test]
fn a_variant_names_its_own_message() {
    let levels = RuleLevels::default();
    let issue = Issue::new(CSP_MISSING).variant("script_src");
    let finding = finding(&issue, &scope(&levels, &[])).expect("finding");
    assert_eq!(
        finding.message,
        "plugin.security-headers.findings.csp.missing_script_src"
    );
}
