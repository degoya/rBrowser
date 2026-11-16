use serde_json::json;

use super::*;
use crate::fake::FakeHost;
use crate::image::tests::png;

const PAGE: &str = "https://a.test/";

fn host() -> FakeHost {
    let mut host = FakeHost::default();
    host.view(1, "Desktop", PAGE).view(2, "Mobile", PAGE);
    host.state(&json!({ "url": PAGE, "secure": true, "sw": { "supported": true } }));
    host
}

fn object(value: &serde_json::Value) -> Map<String, Value> {
    value.as_object().cloned().expect("object")
}

#[test]
fn arguments_must_be_an_object() {
    assert!(parse("").expect("empty").is_empty());
    assert!(parse("null").expect("null").is_empty());
    assert_eq!(
        parse("[1]").expect_err("array").code(),
        Some(codes::INVALID)
    );
}

#[test]
fn check_answers_pages_findings_and_a_summary_and_writes_the_report() {
    let mut host = host();
    let answer = check(&mut host, &object(&json!({ "target": "all" }))).expect("answer");
    assert_eq!(answer["pages"].as_array().expect("pages").len(), 1);
    assert_eq!(answer["pages"][0]["views"], json!([1, 2]));
    assert_eq!(answer["summary"]["error"], 2, "{answer}");
    assert_eq!(answer["summary"]["info"], 2);
    assert_eq!(answer["findings"][0]["rule"], "manifest.missing");
    assert_eq!(answer["findings"][0]["severity"], "error");
    assert_eq!(answer["findings"][0]["data"]["section"], "manifest");
    assert_eq!(answer["written"], true);
    assert!(host.inner.out.contains_key(REPORT_FILE));
    assert_eq!(answer["cut"], false);
}

#[test]
fn a_budget_drops_the_panel_places_first() {
    let mut host = host();
    let answer = check(&mut host, &object(&json!({ "budget": 4096 }))).expect("answer");
    assert!(answer.to_string().len() <= 4096 + 64);
    let bad = check(&mut host, &object(&json!({ "budget": "big" }))).expect_err("bad");
    assert_eq!(bad.param("reason"), Some("budget"));
}

#[test]
fn the_command_reports_the_findings_in_every_view_and_tells_the_panel() {
    let mut host = host();
    let answer = check_command(&mut host).expect("answer");
    assert_eq!(answer["pages"], 1);
    assert_eq!(answer["reported"], true);
    assert_eq!(answer[POST_KEY][0]["target"], "panel");
    let views: Vec<Option<u32>> = host.inner.findings.iter().map(|f| f.view).collect();
    assert_eq!(
        views.len(),
        2 * answer["findings"].as_u64().expect("count") as usize
    );
    assert!(views.contains(&Some(2)));
}

#[test]
fn icon_answers_a_data_uri_of_the_views_origin_only() {
    let mut host = host();
    host.serve("https://a.test/i.png", 200, "image/png", &png(192, 192));
    let answer = icon(
        &mut host,
        &object(&json!({ "url": "https://a.test/i.png" })),
    )
    .expect("icon");
    assert_eq!(answer["width"], 192);
    assert_eq!(answer["format"], "image/png");
    assert!(
        answer["dataUri"]
            .as_str()
            .expect("uri")
            .starts_with("data:image/png;base64,iVBORw0KGgo")
    );
    let other = icon(
        &mut host,
        &object(&json!({ "url": "https://b.test/i.png" })),
    );
    assert_eq!(other.expect_err("origin").param("reason"), Some("origin"));
    let gone = icon(
        &mut host,
        &object(&json!({ "url": "https://a.test/gone.png" })),
    );
    assert_eq!(gone.expect_err("404").param("status"), Some("404"));
    let bad = icon(&mut host, &object(&json!({ "url": "file:///etc/passwd" })));
    assert_eq!(bad.expect_err("url").param("reason"), Some("url"));
    assert_eq!(
        host.fetched(),
        vec!["https://a.test/i.png", "https://a.test/gone.png"]
    );
}

#[test]
fn a_large_icon_comes_without_its_bytes() {
    let mut host = host();
    let mut big = png(512, 512);
    big.resize(MAX_ICON_BYTES + 1, 0);
    host.serve("https://a.test/big.png", 200, "image/png", &big);
    let answer = icon(
        &mut host,
        &object(&json!({ "url": "https://a.test/big.png" })),
    )
    .expect("icon");
    assert_eq!(answer["tooLarge"], true);
    assert_eq!(answer["dataUri"], Value::Null);
    assert_eq!(answer["width"], 512);
}
