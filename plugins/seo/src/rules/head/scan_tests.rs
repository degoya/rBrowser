use super::*;

fn names(tags: &[Tag]) -> Vec<&str> {
    tags.iter().map(|tag| tag.name.as_str()).collect()
}

#[test]
fn tags_skip_comments_and_raw_text_and_read_attributes() {
    let html = r#"<!doctype html><!-- <meta charset="x"> --><html lang=en><head>
<meta charset="utf-8"><script>if (a < b) { document.write("<title>no</title>"); }</script>
<link rel='stylesheet preload' href="/a.css" disabled><TITLE>T <b></TITLE></head>"#;
    let tags = tags(html);
    assert_eq!(
        names(&tags),
        ["html", "head", "meta", "script", "link", "title"]
    );
    assert_eq!(tags[0].attr("lang"), Some("en"));
    assert_eq!(tags[2].attr("charset"), Some("utf-8"));
    assert!(tags[4].has_token("rel", "Stylesheet"));
    assert_eq!(tags[4].attr("href"), Some("/a.css"));
    assert_eq!(tags[4].attr("disabled"), Some(""));
    assert_eq!(
        &html[tags[2].start..tags[2].end],
        r#"<meta charset="utf-8">"#
    );
}

#[test]
fn a_body_element_in_the_head_ends_it() {
    let scan = scan(
        "<html><head><meta charset=utf-8><img src=a.png><title>T</title>\
<link rel=canonical href=/></head><body><p>x</p><meta itemprop=x content=y></body></html>",
    );
    assert_eq!(names(&scan.head), ["meta"]);
    assert_eq!(
        scan.breaker.as_ref().map(|tag| tag.name.as_str()),
        Some("img")
    );
    assert_eq!(names(&scan.body), ["title", "link", "p", "meta"]);
}

#[test]
fn a_document_without_head_tags_still_has_a_head() {
    let scan = scan("<meta charset=utf-8><title>T</title><body><title>again</title>");
    assert_eq!(names(&scan.head), ["meta", "title"]);
    assert_eq!(scan.breaker, None);
    assert_eq!(names(&scan.body), ["title"]);
}

#[test]
fn an_svg_s_title_is_skipped() {
    let scan = scan(
        "<head><title>T</title></head><body><svg><title>Logo</title></svg><svg/><p>x</p></body>",
    );
    assert_eq!(names(&scan.body), ["svg", "svg", "p"]);
}
