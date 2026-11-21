use scraper::{Html, Selector};

use super::*;

fn first<'a>(html: &'a Html, selector: &str) -> scraper::ElementRef<'a> {
    let selector = Selector::parse(selector).expect("selector");
    html.select(&selector).next().expect("element")
}

#[test]
fn the_path_numbers_a_tag_only_among_siblings_of_the_same_tag() {
    let html = Html::parse_document(
        "<html><body><main><p>a</p><img src=a><img src=b id=x class=y></main></body></html>",
    );
    assert_eq!(
        element_path(first(&html, "#x")),
        "html > body > main > img:nth-of-type(2)"
    );
    assert_eq!(element_path(first(&html, "p")), "html > body > main > p");
}

#[test]
fn the_root_is_html_and_head_and_body_need_no_number() {
    let html = Html::parse_document("<title>t</title><p>x</p>");
    assert_eq!(element_path(first(&html, "html")), "html");
    assert_eq!(element_path(first(&html, "title")), "html > head > title");
}

#[derive(serde::Deserialize)]
struct Case {
    name: String,
    html: String,
    path: String,
    #[serde(default)]
    shadow: bool,
}

#[derive(serde::Deserialize)]
struct Cases {
    cases: Vec<Case>,
}

/// The element path every plugin shares (RB-094-10), held to the same cases as the TypeScript
/// one (`plugins/web-common/src/element-path.ts`); html5ever builds no shadow roots, so the
/// shadow cases are the live DOM's only.
#[test]
fn the_path_matches_every_case_of_the_shared_fixture() {
    let cases: Cases = serde_json::from_str(include_str!(
        "../../../tests/fixtures/element-path/cases.json"
    ))
    .expect("the fixture is JSON");
    let mut checked = 0;
    for case in cases.cases.iter().filter(|case| !case.shadow) {
        let html = Html::parse_document(&case.html);
        assert_eq!(
            element_path(first(&html, "[data-target]")),
            case.path,
            "case {:?}",
            case.name
        );
        checked += 1;
    }
    assert!(checked >= 10, "only {checked} cases checked");
}
