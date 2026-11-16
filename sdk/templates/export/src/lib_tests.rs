use super::render;

#[test]
fn every_visited_url_becomes_a_goto() {
    let text = render(r##"{"steps": [{"url": "https://example.org/"}, {"click": "#a"}]}"##)
        .expect("a recording");
    assert!(text.contains("await page.goto(\"https://example.org/\");"));
    assert_eq!(text.matches("page.goto").count(), 1);
}

#[test]
fn a_recording_that_is_no_json_is_refused() {
    assert!(render("not json").is_err());
}
