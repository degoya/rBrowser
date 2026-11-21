use super::*;
use crate::testing::{URL, good, host, serve, view};

fn context(view: Option<u32>, url: Option<&str>) -> CheckContext {
    CheckContext {
        view,
        url: url.map(str::to_owned),
        rules: Vec::new(),
        args: "{}".to_owned(),
    }
}

fn without_hsts() -> Vec<(&'static str, &'static str)> {
    good()
        .into_iter()
        .filter(|(name, _)| *name != "strict-transport-security")
        .collect()
}

#[test]
fn every_view_of_one_page_fetches_it_once_and_finds_once() {
    let mut host = host();
    serve(&mut host, URL, &without_hsts());
    view(&mut host, 1, URL);
    view(&mut host, 2, "https://example.test/#top");
    view(&mut host, 3, URL);
    let findings = check(&mut host, &context(None, None)).expect("findings");
    assert_eq!(host.fetched.len(), 1);
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].rule, "hsts.missing");
    assert_eq!(findings[0].view, None);
    assert_eq!(findings[0].url.as_deref(), Some(URL));
}

#[test]
fn the_runs_view_or_url_is_checked() {
    let mut host = host();
    serve(&mut host, URL, &good());
    serve(&mut host, "https://example.test/other", &without_hsts());
    view(&mut host, 1, URL);
    view(&mut host, 2, "https://example.test/other");
    assert!(
        check(&mut host, &context(Some(1), None))
            .expect("view 1")
            .is_empty()
    );
    assert_eq!(
        check(&mut host, &context(Some(2), None))
            .expect("view 2")
            .len(),
        1
    );
    assert_eq!(
        check(
            &mut host,
            &context(Some(1), Some("https://example.test/other"))
        )
        .expect("url")
        .len(),
        1
    );
    let gone = check(&mut host, &context(Some(9), None)).expect_err("gone");
    assert_eq!(gone.code(), Some(codes::NOT_FOUND));
}

#[test]
fn only_the_rules_asked_for_are_run() {
    let mut host = host();
    serve(&mut host, URL, &[("date", "now")]);
    view(&mut host, 1, URL);
    let mut ctx = context(Some(1), None);
    ctx.rules = vec!["nosniff.missing".to_owned()];
    let findings = check(&mut host, &ctx).expect("findings");
    let rules: Vec<&str> = findings
        .iter()
        .map(|finding| finding.rule.as_str())
        .collect();
    assert_eq!(rules, ["nosniff.missing"]);
}

#[test]
fn a_page_that_fails_is_skipped_among_all_but_fails_alone() {
    let mut host = host();
    serve(&mut host, URL, &without_hsts());
    view(&mut host, 1, URL);
    view(&mut host, 2, "https://filtered.example.test/");
    host.responses.insert(
        "https://filtered.example.test/".to_owned(),
        crate::testing::response(200, &[("content-type", "text/html")]),
    );
    assert_eq!(
        check(&mut host, &context(None, None)).expect("all").len(),
        1
    );
    assert!(check(&mut host, &context(Some(2), None)).is_err());
}

#[test]
fn views_without_a_web_page_are_left_out() {
    let mut host = host();
    view(&mut host, 1, "about:blank");
    assert!(
        check(&mut host, &context(None, None))
            .expect("none")
            .is_empty()
    );
    assert!(host.fetched.is_empty());
}
