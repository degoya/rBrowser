use rb_plugin_types::{CheckFailure, FindingsSummary};

use super::super::testing::{MARKUP, PAGE, fixture, with_context};
use super::*;

fn page_of(file: &FindingsFile) -> String {
    with_context(|context| render(file, context))
}

#[test]
fn the_page_loads_nothing_from_anywhere() {
    let page = page_of(&fixture());
    assert!(page.starts_with("<!doctype html>"));
    assert!(page.contains(
        "<meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; style-src 'unsafe-inline'\">"
    ));
    for loader in [
        "<script", "<link", "<img", "<iframe", "<object", "<embed", "src=", "url(", "@import",
        "<base",
    ] {
        assert!(!page.to_ascii_lowercase().contains(loader), "{loader}");
    }
}

#[test]
fn every_finding_is_a_row_with_breakpoint_engine_page_and_element() {
    let page = page_of(&fixture());
    assert_eq!(page.matches("<tr class=\"finding ").count(), 5);
    assert!(page.contains("<td>mobile</td><td>chromium</td>"), "{page}");
    assert!(page.contains("<td>desktop</td><td>firefox</td>"));
    assert!(page.contains("<code>img.hero</code>"));
    assert!(page.contains(&escape(PAGE)));
    assert!(page.contains("The page has no title"));
}

#[test]
fn errors_come_first() {
    let page = page_of(&fixture());
    let error = page
        .find("<tr class=\"finding error\"")
        .expect("an error row");
    let warning = page
        .find("<tr class=\"finding warning\"")
        .expect("a warning row");
    let info = page
        .find("<tr class=\"finding info\"")
        .expect("an info row");
    assert!(error < warning && warning < info);
}

#[test]
fn every_finding_text_is_escaped() {
    let mut file = fixture();
    file.findings[0].rule_id = "<b>rule</b>".to_owned();
    file.findings[1].element = Some(rb_plugin_types::ElementRef {
        selector: Some("a[title=\"x\"]>'<i>'".to_owned()),
        path: None,
    });
    file.views[0].breakpoint = Some("<mobile>".to_owned());
    file.findings[0].breakpoints = vec!["<mobile>".to_owned()];
    let page = page_of(&file);
    assert!(!page.contains(MARKUP));
    assert!(page.contains("&lt;script&gt;alert(&quot;x&quot;)&lt;/script&gt;"));
    assert!(!page.contains("<b>rule</b>"));
    assert!(page.contains("&lt;b&gt;rule&lt;/b&gt;"));
    assert!(!page.contains("'<i>'"));
    assert!(!page.contains("<mobile>"));
}

#[test]
fn a_help_link_is_only_a_web_page() {
    let page = page_of(&fixture());
    assert!(
        page.contains("<a href=\"https://help.test/alt?a=1&amp;b=2\" rel=\"noreferrer\">help</a>")
    );
    assert!(!page.contains("javascript:"));
}

#[test]
fn the_head_names_the_run_its_summary_and_plugins() {
    let page = page_of(&fixture());
    assert!(page.contains("<title>title run-1700000000000</title>"));
    assert!(page.contains("summary(total=5,error=2,warning=2,info=1)"));
    assert!(page.contains("seo, a11y"));
}

#[test]
fn a_run_without_findings_says_so_and_names_its_failures() {
    let mut file = fixture();
    file.findings.clear();
    file.summary = FindingsSummary::default();
    file.failures.push(CheckFailure {
        plugin: "seo".to_owned(),
        view: None,
        code: None,
        params: Default::default(),
    });
    let page = page_of(&file);
    assert!(page.contains("<p>none</p>"));
    assert!(!page.contains("<table"));
    assert!(page.contains("<li>plugin_failed(plugin=seo,reason=)</li>"));
}
