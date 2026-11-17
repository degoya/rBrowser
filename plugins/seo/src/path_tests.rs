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

#[test]
fn the_selector_prefers_an_id_and_falls_back_to_the_path() {
    let html =
        Html::parse_document("<body><img id=hero src=a><img src=b><img id='a b' src=c></body>");
    assert_eq!(selector_of(first(&html, "#hero")), "#hero");
    assert_eq!(
        selector_of(first(&html, "img[src=b]")),
        "html > body > img:nth-of-type(2)"
    );
    assert_eq!(
        selector_of(first(&html, "img[src=c]")),
        "html > body > img:nth-of-type(3)"
    );
}
