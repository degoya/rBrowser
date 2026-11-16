use super::*;

fn injected(html: &str) -> String {
    String::from_utf8(inject(html.as_bytes())).expect("utf-8")
}

#[test]
fn the_agent_goes_right_after_the_head_tag() {
    let out = injected(
        "<!doctype html><html><HEAD lang=\"en\"><title>x</title></head><body></body></html>",
    );
    assert_eq!(
        out,
        format!(
            "<!doctype html><html><HEAD lang=\"en\">{AGENT_TAG}<title>x</title></head><body></body></html>"
        )
    );
}

#[test]
fn without_a_head_it_goes_after_html_or_first() {
    assert_eq!(
        injected("<html><body>x</body></html>"),
        format!("<html>{AGENT_TAG}<body>x</body></html>")
    );
    assert_eq!(injected("<p>x</p>"), format!("{AGENT_TAG}<p>x</p>"));
}

#[test]
fn a_header_tag_is_no_head() {
    assert_eq!(
        injected("<html><header>x</header></html>"),
        format!("<html>{AGENT_TAG}<header>x</header></html>")
    );
}

#[test]
fn the_tag_loads_the_agent_from_the_portal_only() {
    assert!(AGENT_TAG.contains(&format!("src=\"{AGENT_PATH}\"")));
    assert!(AGENT_PATH.starts_with("/__rbrowser/"));
}

#[test]
fn a_chunked_body_decodes() {
    assert_eq!(
        decode_chunked(b"4\r\nWiki\r\n5;ext=1\r\npedia\r\n0\r\n\r\n").as_deref(),
        Some(&b"Wikipedia"[..])
    );
    assert_eq!(decode_chunked(b"4\r\nWik"), None, "a cut body is no body");
    assert_eq!(decode_chunked(b"zz\r\n"), None);
}

#[test]
fn the_body_is_complete_by_length_chunks_or_close() {
    assert_eq!(complete(b"abc", Some(3), false), Some(3));
    assert_eq!(complete(b"ab", Some(3), false), None);
    assert_eq!(complete(b"1\r\na\r\n0\r\n\r\n", None, true), Some(11));
    assert_eq!(complete(b"1\r\na\r\n", None, true), None);
    assert_eq!(
        complete(b"abc", None, false),
        None,
        "read until the upstream closes"
    );
}

/// A site that writes its own origin into links and asset URLs (`APP_URL`) would send the
/// device to a host it cannot reach.
#[test]
fn absolute_upstream_urls_point_at_the_portal() {
    let html = b"<a href=\"http://localhost:3000/a?x=1\">a</a><script src=\"http://localhost:3000/app.js\"></script>\
                 <a href=\"http://localhost:30001/\">other port</a><a href=\"https://localhost:3000/\">tls</a>\
                 <a href=\"HTTP://LOCALHOST:3000/up\">case</a> http://localhost:3000";
    let out = rewrite_html(html, "http://localhost:3000", "http://192.168.1.5:4312");
    let out = String::from_utf8(out).expect("utf-8");
    assert_eq!(
        out,
        "<a href=\"http://192.168.1.5:4312/a?x=1\">a</a><script src=\"http://192.168.1.5:4312/app.js\"></script>\
         <a href=\"http://localhost:30001/\">other port</a><a href=\"https://localhost:3000/\">tls</a>\
         <a href=\"http://192.168.1.5:4312/up\">case</a> http://192.168.1.5:4312"
    );
    assert_eq!(
        rewrite_html(b"<p>none</p>", "http://a:1", "http://b:2"),
        b"<p>none</p>"
    );
}
