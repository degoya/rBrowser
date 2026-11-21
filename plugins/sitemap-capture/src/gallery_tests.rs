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
    assert!(html.contains("main:has(#bp-1:checked) .sheet .cell:not(.bp-1){display:none}"));
    assert!(
        !html.contains("~.sheet"),
        "the radios are no siblings of the sheet"
    );
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

/// The open elements around the first tag whose text contains `marker`, outermost first: a tag
/// stack over the gallery's `<body>` (its own markup: no comments, no raw text, every `<` a tag).
fn ancestors(html: &str, marker: &str) -> Vec<String> {
    const VOID: [&str; 5] = ["meta", "input", "img", "br", "hr"];
    let body = html
        .split("<body>")
        .nth(1)
        .and_then(|body| body.split("</body>").next())
        .expect("a body");
    let mut stack: Vec<String> = Vec::new();
    let mut rest = body;
    while let Some(start) = rest.find('<') {
        let end = rest[start..].find('>').expect("a closed tag") + start;
        let tag = &rest[start + 1..end];
        rest = &rest[end + 1..];
        if let Some(name) = tag.strip_prefix('/') {
            let open = stack.pop().expect("an open element");
            assert_eq!(open, name, "nesting");
            continue;
        }
        let name: String = tag
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric())
            .collect();
        if tag.contains(marker) {
            return stack;
        }
        if !VOID.contains(&name.as_str()) {
            stack.push(name);
        }
    }
    panic!("no element with {marker}");
}

#[test]
fn every_filter_rule_reaches_the_sheet_from_an_ancestor_of_its_radio() {
    let html = render(&sample(), &BTreeMap::new());
    let sheet = ancestors(&html, "class=\"sheet\"");
    for radio in ["bp-0", "bp-1", "st-ok", "st-problem"] {
        let around = ancestors(&html, &format!("id=\"{radio}\""));
        assert!(
            around.contains(&"main".to_owned()) && sheet.contains(&"main".to_owned()),
            "{radio} and the sheet both sit in <main>: {around:?} / {sheet:?}"
        );
        assert!(
            html.contains(&format!("main:has(#{radio}:checked) .sheet ")),
            "the rule of {radio} starts at <main>"
        );
    }
    assert!(
        html.contains("main:has(#st-ok:checked) .sheet tr.problem,main:has(#st-problem:checked) .sheet tr.fine{display:none}")
    );
}

#[test]
fn the_thumbnails_take_the_width_asked_for_and_open_the_large_view() {
    let html = render_sized(&sample(), &BTreeMap::new(), 480);
    assert!(html.contains(".sheet img{width:480px}"));
    assert!(html.contains("<a href=\"#v-0-0\"><img src=\"index/mobile.png\""));
    let default = render(&sample(), &BTreeMap::new());
    assert!(default.contains(&format!(
        ".sheet img{{width:{}px}}",
        default_thumbnail_width()
    )));
}

#[test]
fn a_thumbnail_width_is_one_of_the_schemas_choices_else_its_default() {
    let schema: serde_json::Value = serde_json::from_str(SCHEMA).expect("the schema");
    let key = &schema["properties"]["thumbnailWidth"];
    let default = key["default"].as_u64().expect("a default");
    assert_eq!(u64::from(default_thumbnail_width()), default);
    for choice in key["enum"].as_array().expect("choices") {
        let choice = choice.as_u64().expect("a number");
        assert_eq!(
            u64::from(thumbnail_width(Some(&choice.to_string()))),
            choice
        );
    }
    for odd in [
        None,
        Some("0"),
        Some("333"),
        Some("\"wide\""),
        Some("100000"),
        Some(""),
    ] {
        assert_eq!(u64::from(thumbnail_width(odd)), default, "{odd:?}");
    }
}

#[test]
fn the_export_takes_the_apps_colours_in_light_and_dark() {
    let html = render(&sample(), &BTreeMap::new());
    let style = html
        .split("<style>")
        .nth(1)
        .and_then(|rest| rest.split("</style>").next())
        .expect("a style");
    assert!(
        style.contains("--primary:#115e59"),
        "teal, the app's primary"
    );
    assert!(style.contains("@media (prefers-color-scheme:dark){:root{"));
    assert!(style.contains("--primary:#2dd4bf"));
    assert!(!style.contains("url("), "no font or image from anywhere");
}
