use serde_json::json;

use super::*;
use crate::testing::{URL, good, host, serve, view, with_environments};

const SECRET: &str = "VIEW-HELD-SECRET";

#[test]
fn a_good_page_has_no_findings_and_names_its_environment() {
    let mut host = host();
    serve(&mut host, URL, &good());
    with_environments(&mut host, &[("prod", "https://example.test")]);
    let envs = environments::read(&mut host);
    let checked = check_url(&mut host, URL, &envs, &Settings::default(), true).expect("checked");
    assert!(checked.issues.is_empty(), "{:?}", checked.issues);
    assert_eq!(checked.environment.as_deref(), Some("prod"));
}

#[test]
fn the_cookies_of_a_view_showing_the_origin_are_held_to_the_rules() {
    let mut host = host();
    serve(&mut host, URL, &good());
    view(&mut host, 4, "https://example.test/account");
    host.cdp_answer = json!({ "cookies": [
        { "name": "sid", "value": SECRET, "domain": "example.test", "path": "/", "secure": true, "httpOnly": false, "sameSite": "Lax" }
    ]})
    .to_string();
    let checked = check_url(&mut host, URL, &[], &Settings::default(), true).expect("checked");
    assert_eq!(host.cdp_sent.len(), 1);
    let (sent_view, method, params) = &host.cdp_sent[0];
    assert_eq!((*sent_view, method.as_str()), (4, "Network.getCookies"));
    assert_eq!(
        params,
        &json!({ "urls": ["https://example.test/account"] }).to_string()
    );
    let findings = checked.findings(&[], &RuleLevels::default());
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].rule, "cookie.flags");
    assert!(!format!("{findings:?}").contains(SECRET));
    assert!(!format!("{checked:?}").contains(SECRET));
}

#[test]
fn a_view_of_another_origin_is_not_asked_and_the_comparison_asks_none() {
    let mut host = host();
    serve(&mut host, URL, &good());
    view(&mut host, 1, "https://other.test/");
    check_url(&mut host, URL, &[], &Settings::default(), true).expect("checked");
    assert!(host.cdp_sent.is_empty());
    view(&mut host, 2, URL);
    check_url(&mut host, URL, &[], &Settings::default(), false).expect("checked");
    assert!(host.cdp_sent.is_empty());
}

#[test]
fn a_refused_cookie_read_leaves_the_answer_cookies() {
    let mut host = host();
    host.cdp_domains.clear();
    let mut headers = good();
    headers.push(("set-cookie", "sid=x; Path=/"));
    serve(&mut host, URL, &headers);
    view(&mut host, 1, URL);
    let checked = check_url(&mut host, URL, &[], &Settings::default(), true).expect("checked");
    assert_eq!(checked.cookies.len(), 1);
}
