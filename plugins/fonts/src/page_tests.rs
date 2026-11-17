use serde_json::json;

use super::*;
use crate::fake::FakeHost;

#[test]
fn parses_the_committed_fixture_answer() {
    let fonts = PageFonts::parse(include_str!("../tests/fixtures/narrow.json")).expect("fixture");
    assert_eq!(fonts.url, "http://localhost:8080/fonts/index.html");
    assert_eq!(fonts.faces.len(), 6);
    assert_eq!(fonts.faces[1].display, "auto");
    assert_eq!(fonts.faces[3].src[0].format, "truetype");
    assert!(fonts.preloaded("http://localhost:8080/fonts/inter-400.woff2"));
    assert_eq!(fonts.resources[0].size, 17156);
    assert_eq!(fonts.small_inputs[0].size, 14.0);
    let subset = fonts
        .usages
        .iter()
        .find(|usage| usage.family == "Inter ASCII")
        .expect("subset usage");
    assert_eq!(subset.samples[0].non_ascii.chars().count(), 5);
}

#[test]
fn caps_lists_cuts_texts_and_makes_numbers_finite() {
    let long = "x".repeat(5000);
    let answer = json!({
        "url": long,
        "faces": vec![json!({ "family": long, "src": vec![json!({ "url": "a" }); 40] }); 300],
        "smallText": vec![json!({ "size": "nonsense", "lineHeight": 1e300 }); 80],
        "resources": [{ "url": "f.woff2", "size": -5 }],
        "preloads": ["a", 3, null],
    });
    let fonts = PageFonts::parse(&answer.to_string()).expect("parsed");
    assert_eq!(fonts.url.chars().count(), MAX_URL);
    assert_eq!(fonts.faces.len(), MAX_FACES);
    assert_eq!(fonts.faces[0].family.chars().count(), MAX_TEXT);
    assert_eq!(fonts.faces[0].src.len(), 16);
    assert_eq!(fonts.small_text.len(), MAX_CANDIDATES);
    assert_eq!(fonts.small_text[0].size, 0.0);
    assert!(fonts.small_text[0].line_height.is_finite());
    assert_eq!(fonts.resources[0].size, 0);
    assert_eq!(fonts.preloads, vec![json!("a")]);
}

#[test]
fn refuses_what_is_no_object() {
    for answer in ["[]", "null", "nonsense", "\"text\""] {
        let failure = PageFonts::parse(answer).expect_err("refused");
        assert_eq!(failure.code(), Some(codes::BAD_OUTPUT));
    }
}

#[test]
fn the_script_carries_the_thresholds_as_json_data() {
    let settings = Settings {
        min_text_px: 13.0,
        ..Settings::default()
    };
    let script = script(&settings);
    assert!(script.starts_with("(() => {"));
    assert!(script.contains("var FontsPage"));
    assert!(script.ends_with(
        "return FontsPage.run({\"minTextPx\":13.0,\"minInputPx\":16.0,\"minLineHeight\":1.2}); })()"
    ));
}

#[test]
fn mark_sends_the_family_as_data_and_reads_the_count() {
    let mut host = FakeHost::default();
    host.view(1, "Desktop", 1280, "http://x/");
    host.marks.insert(1, json!({ "count": 4 }));
    let family = "Inter\"); alert(1); (\"";
    assert_eq!(
        mark(&mut host, 1, family, Some(600), None).expect("marked"),
        4
    );
    let (_, script) = host.evaluated.last().expect("evaluated");
    assert!(script.contains(
        r#"FontsPage.mark({"family":"Inter\"); alert(1); (\"","weight":600,"style":null})"#
    ));
    clear(&mut host, 1).expect("cleared");
    assert!(
        host.evaluated
            .last()
            .expect("evaluated")
            .1
            .contains("FontsPage.clear(null)")
    );
}
