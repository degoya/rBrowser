use super::*;

fn run(json: &str) -> Run {
    serde_json::from_str(json).expect("a run")
}

fn sample() -> Run {
    run(r#"{
        "runId": "2026-11-11_00-00-00",
        "source": "https://www.example.test/",
        "state": "finished",
        "breakpoints": [
            {"name": "mobile", "label": "Mobile", "width": 375, "height": 812},
            {"name": "desktop", "label": "", "width": 1440, "height": 900}
        ],
        "pages": [
            {"url": "https://www.example.test/", "slug": "index", "status": 200,
             "shots": {"mobile": {"state": "done"}, "desktop": {"state": "done",
                       "compare": {"status": "differs", "ratio": 0.0123}}}},
            {"url": "https://www.example.test/missing", "slug": "missing", "status": 404,
             "shots": {"mobile": {"state": "done"}, "desktop": {"state": "done"}}},
            {"url": "https://www.example.test/old", "slug": "old", "status": 200,
             "finalUrl": "https://www.example.test/new",
             "shots": {"mobile": {"state": "failed", "error": "timeout"}, "desktop": {"state": "pending"}}}
        ]
    }"#)
}

#[test]
fn the_contact_sheet_has_every_page_and_breakpoint() {
    let html = render(&sample(), &BTreeMap::new());
    assert!(html.starts_with("<!doctype html>"));
    assert!(
        html.contains("<img src=\"index/mobile.png\" alt=\"https://www.example.test/ at Mobile\"")
    );
    assert!(html.contains("<img src=\"missing/desktop.png\""));
    assert!(
        html.contains("desktop <small>1440&times;900</small>"),
        "an empty label shows the name"
    );
    assert!(html.contains("Failed: timeout"));
    assert!(html.contains("Not captured yet"));
    assert!(html.contains("differs 1.23 %"));
    assert!(html.contains("3 pages in 2 breakpoints"));
}

#[test]
fn the_error_list_names_the_404_the_failure_and_the_redirect() {
    let html = render(&sample(), &BTreeMap::new());
    let errors = html
        .split("<section id=\"errors\">")
        .nth(1)
        .expect("errors");
    let errors = errors.split("</section>").next().expect("end");
    assert!(errors.contains("HTTP 404"));
    assert!(errors.contains("https://www.example.test/missing"));
    assert!(errors.contains("Redirect"));
    assert!(errors.contains("to https://www.example.test/new"));
    assert!(errors.contains("Failed</td><td>mobile: timeout"));
    let clean = run(
        r#"{"breakpoints":[{"name":"m"}],"pages":[{"url":"https://a.test/","slug":"index","status":200,"shots":{"m":{"state":"done"}}}]}"#,
    );
    assert!(render(&clean, &BTreeMap::new()).contains("No errors."));
}

#[test]
fn filters_and_the_large_view_need_no_script() {
    let html = render(&sample(), &BTreeMap::new());
    assert!(!html.to_ascii_lowercase().contains("<script"));
    assert!(html.contains("default-src 'none'"));
    assert!(html.contains("id=\"bp-0\"") && html.contains("id=\"bp-1\""));
    assert!(html.contains("#bp-1:checked~.sheet .cell:not(.bp-1){display:none}"));
    assert!(html.contains("id=\"st-problem\""));
    assert!(html.contains("<tr class=\"problem\">"));
    assert!(html.contains("<a href=\"#v-0-0\">"));
    assert!(html.contains("<section class=\"view\" id=\"v-0-0\""));
    assert!(html.contains("<a href=\"#v-0-1\">Next</a>"));
    assert!(html.contains("<a href=\"#v-0-0\">Previous</a>"));
    assert!(
        !html.contains("id=\"v-2-0\""),
        "a failed shot has no large view"
    );
}

#[test]
fn hostile_sitemap_text_is_escaped_and_never_a_link_or_path() {
    let hostile = run(r#"{
        "runId": "<b>run</b>",
        "source": "\"><script>alert(1)</script>",
        "breakpoints": [
            {"name": "mobile", "label": "<img src=x onerror=alert(1)>", "width": 1, "height": 1},
            {"name": "../../evil", "label": "bad", "width": 1, "height": 1}
        ],
        "pages": [
            {"url": "javascript:alert(1)", "slug": "../../etc", "status": 500,
             "shots": {"mobile": {"state": "done"}}},
            {"url": "https://a.test/?q=\"><script>x</script>&a='b'", "slug": "ok",
             "shots": {"mobile": {"state": "failed", "error": "<iframe src=//evil>"}}}
        ]
    }"#);
    let html = render(&hostile, &BTreeMap::new());
    assert!(!html.contains("<script>"));
    assert!(!html.contains("<iframe"));
    assert!(!html.contains("<b>run"));
    assert!(!html.contains("<img src=x"));
    assert!(html.contains("&lt;img src=x onerror=alert(1)&gt;"));
    assert!(!html.contains("href=\"javascript:"));
    assert!(
        !html.contains("../../etc"),
        "a slug that is no plain name gives no image"
    );
    assert!(
        !html.contains("evil.png") && !html.contains("bp-1"),
        "nor does such a breakpoint"
    );
    assert!(html.contains("&quot;&gt;&lt;script&gt;"));
    assert!(html.contains("&#39;b&#39;"));
    assert_eq!(sanitized("a\u{0}b'"), "a\u{fffd}b&#39;");
}

#[test]
fn labels_replace_the_english_defaults() {
    let mut labels = BTreeMap::new();
    labels.insert("title".to_owned(), "Sitemap-Aufnahme <x>".to_owned());
    let html = render(&sample(), &labels);
    assert!(html.contains("<h1>Sitemap-Aufnahme &lt;x&gt;</h1>"));
}

#[test]
fn the_defaults_are_the_english_catalogue() {
    let catalogue: serde_json::Value =
        serde_json::from_str(include_str!("../locales/en.json")).expect("the catalogue");
    for (name, text) in LABELS {
        let found = &catalogue["plugin"]["sitemap-capture"]["gallery"][name];
        assert_eq!(found, text, "gallery.{name}");
    }
}
