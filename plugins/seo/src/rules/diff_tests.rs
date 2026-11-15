use crate::testing::{check, rendered, serve, short};

const URL: &str = "https://shop.test/";

#[test]
fn a_canonical_set_only_by_javascript_is_reported() {
    let raw = "<html><head><title>T</title></head><body><h1>A</h1></body></html>";
    let dom = r#"<html><head><title>T</title><link rel="canonical" href="https://shop.test/"></head><body><h1>B</h1></body></html>"#;
    let mut host = serve(URL, raw, &[]);
    let view = rendered(URL, dom);
    let findings = check(&mut host, URL, Some(&view));
    assert_eq!(
        short(&findings, "render"),
        [
            "render.differs render.js_only",
            "render.differs render.changed"
        ]
    );
    let js_only = findings
        .iter()
        .find(|finding| finding.message == "plugin.seo.findings.render.js_only")
        .expect("js only");
    assert!(
        js_only
            .params
            .contains(&("field".to_owned(), "canonical".to_owned()))
    );
    assert!(
        findings.iter().all(|finding| finding.view == Some(1)),
        "a rendered page's findings name its view"
    );
}

#[test]
fn a_view_on_another_page_is_not_compared() {
    let raw = "<html><head><title>T</title></head></html>";
    let mut host = serve(URL, raw, &[]);
    let view = rendered("https://shop.test/other", "<html><head></head></html>");
    let findings = check(&mut host, URL, Some(&view));
    assert!(short(&findings, "render").is_empty());
    assert!(findings.iter().all(|finding| finding.view.is_none()));
}
