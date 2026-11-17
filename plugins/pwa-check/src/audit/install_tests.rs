use serde_json::json;

use super::*;
use crate::fake::FakeHost;

#[test]
fn every_error_is_a_finding_with_its_arguments() {
    let mut host = FakeHost::default();
    host.inner.cdp_answer = json!({ "installabilityErrors": [
        { "errorId": "manifest-missing-suitable-icon", "errorArguments": [
            { "name": "minimum-icon-size-in-pixels", "value": "144" } ] },
        { "errorId": "no-icon-available", "errorArguments": [] }
    ] })
    .to_string();
    let mut sink = Sink::new(&[], "https://a.test/", Some(4));
    let report = check(&mut host, 4, &mut sink);
    assert!(report.checked);
    assert_eq!(report.errors.len(), 2);
    assert_eq!(
        host.inner.cdp_sent[0],
        (4, METHOD.to_owned(), "{}".to_owned())
    );
    let first = &sink.findings[0];
    assert_eq!(first.rule, INSTALL_ERROR);
    assert_eq!(
        first.path.as_deref(),
        Some("pwa:install/manifest-missing-suitable-icon")
    );
    assert_eq!(first.params[1].1, "minimum-icon-size-in-pixels: 144");
}

#[test]
fn a_refusal_is_reported_not_raised() {
    let mut host = FakeHost::default();
    host.inner.cdp_domains.clear();
    let mut sink = Sink::new(&[], "https://a.test/", Some(1));
    let report = check(&mut host, 1, &mut sink);
    assert!(!report.checked);
    assert_eq!(report.failure.as_deref(), Some(codes::CDP_DOMAIN_DENIED));
    assert!(sink.findings.is_empty());
}

#[test]
fn not_asked_when_the_rule_is_off() {
    let mut host = FakeHost::default();
    let only = vec!["sw.missing".to_owned()];
    let mut sink = Sink::new(&only, "https://a.test/", Some(1));
    let report = check(&mut host, 1, &mut sink);
    assert!(!report.checked && report.failure.is_none());
    assert!(host.inner.cdp_sent.is_empty());
}
