use super::*;

#[test]
fn squash_collapses_white_space_and_cuts_long_text() {
    assert_eq!(squash("  Lamp \n\t LED  "), "Lamp LED");
    assert_eq!(squash(&"a".repeat(MAX_TEXT + 10)).len(), MAX_TEXT);
}

#[test]
fn the_crate_s_selectors_parse() {
    let html = scraper::Html::parse_document("<p itemscope>x</p>");
    assert_eq!(html.select(&sel("[itemscope]")).count(), 1);
}
