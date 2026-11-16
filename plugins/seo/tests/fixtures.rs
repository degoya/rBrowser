//! The acceptance fixtures of RB-060-08 (`tests/fixtures/site/seo/`): each page served through a
//! `MemoryHost` at `https://fixture.test/<folder>/`, its rendered DOM answered by `evaluate`,
//! checked as `checks_run` would, its findings held to the snapshot in `tests/snapshots/`.
//! `RB_UPDATE_SNAPSHOTS=1 cargo test` writes the snapshots anew (review the diff). The site
//! serves a `llms.txt`, a `security.txt` and the share image in order (RB-080-11), so these
//! snapshots hold only the findings of their own fixture.

mod common;

use common::{ORIGIN, check, fixture, html, lines, rendered, site, snapshot};
use rb_plugin_api::Severity;
use serde_json::json;

#[test]
fn the_image_fixture_finds_every_alt_case() {
    let page = fixture("images/index.html");
    let mut host = site("images", html(page.clone(), &[]));
    host.evaluate_answer = rendered(
        &format!("{ORIGIN}/images/"),
        &page,
        json!([{ "path": "html > body > main > div", "url": "/img/hero.jpg", "width": 1200, "height": 480 }]),
    );
    let findings = check(&mut host, "images");
    assert!(findings.iter().all(|finding| finding.view == Some(1)));
    snapshot("images", &findings);
}

#[test]
fn a_canonical_set_only_by_javascript_is_the_one_finding() {
    let mut host = site("canonical-js", html(fixture("canonical-js/raw.html"), &[]));
    host.evaluate_answer = rendered(
        &format!("{ORIGIN}/canonical-js/"),
        &fixture("canonical-js/rendered.html"),
        json!([]),
    );
    let findings = check(&mut host, "canonical-js");
    snapshot("canonical-js", &findings);
    assert_eq!(findings.len(), 1);
    assert!(
        findings[0]
            .params
            .contains(&("field".to_owned(), "canonical".to_owned()))
    );
}

#[test]
fn two_h1_a_skipped_level_and_noindex_by_header_are_found() {
    let page = fixture("broken/index.html");
    let mut host = site(
        "broken",
        html(page.clone(), &[("x-robots-tag", "noindex, nofollow")]),
    );
    host.evaluate_answer = rendered(&format!("{ORIGIN}/broken/"), &page, json!([]));
    snapshot("broken", &check(&mut host, "broken"));
}

#[test]
fn the_clean_fixture_has_no_finding() {
    let page = fixture("clean/index.html");
    let mut host = site("clean", html(page.clone(), &[]));
    host.evaluate_answer = rendered(&format!("{ORIGIN}/clean/"), &page, json!([]));
    let findings = check(&mut host, "clean");
    assert!(
        findings
            .iter()
            .all(|finding| finding.severity != Severity::Error),
        "{}",
        lines(&findings)
    );
    snapshot("clean", &findings);
    assert!(findings.is_empty());
}
