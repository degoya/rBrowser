use rb_plugin_api::codes;
use serde_json::json;

use super::*;
use crate::fake::FakeHost;

fn image(selector: &str) -> Value {
    json!({
        "selector": selector, "path": "1,HTML,1,BODY,0,IMG", "src": "https://example.test/a.jpg",
        "currentSrc": "https://example.test/a-800.jpg", "srcset": "a-800.jpg 800w", "sizes": "",
        "picture": false, "source": null, "sourceWithoutSizes": false, "loading": "lazy",
        "fetchpriority": "", "rendered": { "width": 400, "height": 300 },
        "natural": { "width": 800, "height": 600 }, "file": { "width": 800, "height": 600 },
        "fileFrom": "probe", "vector": false, "loaded": true, "visible": true, "lcp": false,
        "needed": { "width": 400, "height": 300 }, "ratio": 2,
        "issues": [{ "rule": "oversized", "params": { "dpr": "1", "ratio": "2" } }]
    })
}

fn answer(images: Vec<Value>) -> String {
    json!({
        "url": "https://example.test/", "dpr": 2, "viewport": { "width": 390, "height": 844 },
        "images": images, "more": 0, "lcpKnown": true
    })
    .to_string()
}

#[test]
fn an_answer_is_read_with_its_images_and_issues() {
    let page = parse(&answer(vec![image("#a")])).expect("read");
    assert_eq!(page.url, "https://example.test/");
    assert!((page.dpr - 2.0).abs() < f64::EPSILON);
    assert!(page.lcp_known);
    let first = &page.images[0];
    assert_eq!(first.selector, "#a");
    assert_eq!(first.current_src, "https://example.test/a-800.jpg");
    assert_eq!(first.file_from.as_deref(), Some("probe"));
    assert_eq!(first.ratio, Some(2.0));
    assert_eq!(first.issues[0].rule, "oversized");
    assert_eq!(first.issues[0].params["ratio"], "2");
}

#[test]
fn an_answer_is_held_to_the_limits() {
    let mut odd = image(&"x".repeat(MAX_TEXT + 50));
    odd["issues"] = json!([
        { "rule": "oversized", "params": { "dpr": "1", "script": "<b>" } },
        { "rule": "made-up", "params": {} },
        { "rule": "lcp", "variant": "sneaky", "params": {} },
        { "rule": "lcp", "variant": "lazy", "params": { "dpr": "y".repeat(500) } }
    ]);
    odd["rendered"] = json!({ "width": -4, "height": 10 });
    odd["fileFrom"] = json!("guess");
    let mut images = vec![odd];
    images.extend((0..MAX_IMAGES + 2).map(|index| image(&format!("#i{index}"))));
    let page = parse(&answer(images)).expect("read");
    assert_eq!(page.images.len(), MAX_IMAGES);
    assert_eq!(page.more, 3);
    let first = &page.images[0];
    assert_eq!(first.selector.chars().count(), MAX_TEXT);
    assert!(first.rendered.width.abs() < f64::EPSILON);
    assert_eq!(first.file_from, None);
    let kept: Vec<_> = first
        .issues
        .iter()
        .map(|issue| (issue.rule.as_str(), issue.variant.as_deref()))
        .collect();
    assert_eq!(kept, [("oversized", None), ("lcp", Some("lazy"))]);
    assert!(!first.issues[0].params.contains_key("script"));
    assert_eq!(
        first.issues[1].params["dpr"].chars().count(),
        rules::MAX_PARAM_CHARS
    );
}

#[test]
fn an_answer_that_is_no_page_is_bad_output() {
    for bad in ["null", "[]", "\"text\"", "not json"] {
        let failure = parse(bad).expect_err(bad);
        assert_eq!(failure.code(), Some(codes::BAD_OUTPUT), "{bad}");
    }
}

#[test]
fn the_request_carries_the_view_density_and_the_settings() {
    let settings = Settings {
        oversize_factor: 2.0,
        undersize_factor: 0.8,
        check_lcp: false,
    };
    let asked = request(3.0, &settings);
    assert_eq!(
        asked,
        json!({ "dpr": 3.0, "oversizeFactor": 2.0, "undersizeFactor": 0.8, "checkLcp": false })
    );
    // A view without a density leaves it to the page.
    for dpr in [0.0, -1.0, f32::NAN] {
        assert!(request(dpr, &settings).get("dpr").is_none());
    }
    let text = script(&asked);
    assert!(text.contains("var ResponsiveImagesCollect"));
    assert!(text.ends_with(&format!(
        "return ResponsiveImagesCollect.run({asked}); }})()"
    )));
}

#[test]
fn collect_evaluates_the_collector_in_the_view() {
    let mut host = FakeHost::default();
    host.view(4, "Mobile", "https://example.test/").page(
        4,
        &serde_json::from_str(&answer(vec![image("#a")])).expect("json"),
    );
    let page = collect(&mut host, 4, 3.0, &Settings::default()).expect("collected");
    assert_eq!(page.images.len(), 1);
    assert_eq!(host.evaluated[0].0, 4);
    assert!(host.evaluated[0].1.contains("\"dpr\":3.0"));
    let failure = collect(&mut host, 9, 1.0, &Settings::default()).expect_err("no view");
    assert_eq!(failure.code(), Some(codes::NOT_FOUND));
}
