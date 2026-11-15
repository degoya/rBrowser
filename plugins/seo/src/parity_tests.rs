use std::collections::BTreeMap;

use rb_plugin_api::test_support::MemoryHost;
use rb_plugin_api::{Capability, ViewInfo};
use serde_json::json;

use super::*;
use crate::testing::ViewsHost;

const URL: &str = "https://a.test/p/";

fn view(id: ViewId, breakpoint: &str, width: u32, engine: &str, url: &str) -> ViewInfo {
    ViewInfo {
        id,
        label: breakpoint.to_uppercase(),
        breakpoint: breakpoint.to_owned(),
        engine: engine.to_owned(),
        width,
        url: url.to_owned(),
        ..ViewInfo::default()
    }
}

fn answer(head: &str, body: &str, text: &str) -> String {
    json!({
        "url": URL,
        "html": format!("<html><head>{head}</head><body>{body}</body></html>"),
        "text": text,
    })
    .to_string()
}

const HEAD: &str = r#"<title>Bikes</title><link rel="canonical" href="https://a.test/p/">"#;
const TEXT: &str = "Red bikes and blue bikes for the beach and the town";

fn host(desktop_head: &str, desktop_body: &str, desktop_text: &str) -> ViewsHost {
    let mut inner = MemoryHost::new()
        .grant(Capability::Views)
        .grant(Capability::Evaluate);
    inner.views = vec![
        view(1, "desktop", 1280, "chromium", URL),
        view(2, "mobile", 375, "chromium", URL),
        view(3, "tablet", 768, "firefox", URL),
        view(4, "wide", 1920, "chromium", "https://a.test/other/"),
    ];
    let answers = BTreeMap::from([
        (1, answer(desktop_head, desktop_body, desktop_text)),
        (2, answer(HEAD, "<p>x</p>", TEXT)),
    ]);
    ViewsHost { inner, answers }
}

fn run_parity(host: &mut ViewsHost, config: &Config) -> (Option<ParityReport>, Vec<String>) {
    let enabled = Vec::new();
    let mut sink = Sink::new(config, &enabled);
    let url = Url::parse(URL).expect("url");
    let report = run(host, config, &url, &mut sink);
    let found = sink
        .findings
        .iter()
        .map(|f| {
            let get = |key: &str| {
                f.params
                    .iter()
                    .find(|(k, _)| k == key)
                    .map(|(_, v)| v.clone())
                    .unwrap_or_default()
            };
            format!(
                "{} view={} {} vs {}: {} | {}",
                f.rule,
                f.view.unwrap_or_default(),
                get("breakpoint"),
                get("reference"),
                get("value"),
                get("reference_value")
            )
        })
        .collect();
    (report, found)
}

#[test]
fn equal_views_have_no_finding_and_only_chromium_views_of_the_page_count() {
    let mut host = host(HEAD, "<p>y</p>", TEXT);
    let (report, found) = run_parity(&mut host, &Config::default());
    assert!(found.is_empty(), "{found:?}");
    let report = report.expect("two views");
    assert_eq!(report.reference, 2, "the narrowest view by default");
    let views: Vec<ViewId> = report.views.iter().map(|v| v.view).collect();
    assert_eq!(views, [1, 2]);
    let evaluated: Vec<ViewId> = host.inner.evaluated.iter().map(|(v, _)| *v).collect();
    assert_eq!(evaluated, [1, 2]);
}

#[test]
fn an_h1_only_on_desktop_and_json_ld_only_on_mobile_are_two_findings() {
    let mut host = host(HEAD, "<h1>Bikes</h1>", TEXT);
    host.answers.insert(
        2,
        answer(
            &format!(
                r#"{HEAD}<script type="application/ld+json">{{"@context":"https://schema.org","@type":["Product","Thing"]}}</script>"#
            ),
            "",
            TEXT,
        ),
    );
    let (_, found) = run_parity(&mut host, &Config::default());
    assert_eq!(
        found,
        [
            "parity.h1 view=1 desktop vs mobile: Bikes | -",
            "parity.structured_data view=1 desktop vs mobile: - | Product, Thing",
        ]
    );
}

#[test]
fn title_canonical_hreflang_and_text_differences_are_found() {
    let head = r#"<title>Bikes!</title><link rel="canonical" href="/q/">
        <link rel="alternate" hreflang="de" href="https://a.test/de/">"#;
    let mut host = host(head, "", "Something else entirely about cars");
    let (_, found) = run_parity(&mut host, &Config::default());
    let rules: Vec<&str> = found
        .iter()
        .map(|line| line.split(' ').next().unwrap_or_default())
        .collect();
    assert_eq!(
        rules,
        [
            "parity.title",
            "parity.canonical",
            "parity.hreflang",
            "parity.text"
        ]
    );
}

#[test]
fn the_reference_view_follows_the_setting() {
    let mut host = host(HEAD, "<h1>Bikes</h1>", TEXT);
    let config = Config {
        parity_reference: "Desktop".to_owned(),
        ..Config::default()
    };
    let (report, found) = run_parity(&mut host, &config);
    assert_eq!(report.expect("views").reference, 1);
    assert_eq!(found, ["parity.h1 view=2 mobile vs desktop: - | Bikes"]);
}

#[test]
fn similar_text_passes_the_threshold() {
    assert!((similarity("a b c d", "a b c d") - 1.0).abs() < f64::EPSILON);
    assert!((similarity("A, b. C d", "a b c e") - 0.6).abs() < 1e-9);
    assert!((similarity("", "") - 1.0).abs() < f64::EPSILON);
    assert!(similarity("a", "").abs() < f64::EPSILON);
}

#[test]
fn one_view_is_no_comparison() {
    let mut host = host(HEAD, "", TEXT);
    host.inner.views.truncate(1);
    let (report, found) = run_parity(&mut host, &Config::default());
    assert!(report.is_none());
    assert!(found.is_empty());
}
