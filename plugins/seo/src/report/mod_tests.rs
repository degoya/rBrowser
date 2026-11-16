use rb_plugin_api::Capability;
use rb_plugin_api::test_support::MemoryHost;

use super::*;

fn outcome() -> Outcome {
    Outcome {
        url: "https://shop.test/".to_owned(),
        pages: vec![PageSummary {
            final_url: "https://shop.test/".to_owned(),
            status: Some(200),
            title: Some("<Shop>".to_owned()),
            ..PageSummary::default()
        }],
        images: vec![ImageEntry {
            src: "a.jpg\"><script>".to_owned(),
            alt: None,
            selector: "img".to_owned(),
            path: "html > body > img".to_owned(),
            width: None,
            height: None,
            status: vec!["images.alt_missing".to_owned()],
        }],
        findings: vec![
            Finding {
                rule: "headings.skipped".to_owned(),
                severity: Severity::Warning,
                message: "plugin.seo.findings.headings.skipped".to_owned(),
                params: vec![
                    ("from".to_owned(), "h2".to_owned()),
                    ("to".to_owned(), "h4".to_owned()),
                    ("text".to_owned(), "x".to_owned()),
                ],
                ..Finding::default()
            },
            Finding {
                rule: "images.alt_missing".to_owned(),
                severity: Severity::Error,
                message: "plugin.seo.findings.images.alt_missing".to_owned(),
                params: vec![("src".to_owned(), "a.jpg".to_owned())],
                ..Finding::default()
            },
        ],
        ..Outcome::default()
    }
}

#[test]
fn run_ids_are_utc_times_with_the_view() {
    assert_eq!(run_id(0, None), "19700101-000000");
    assert_eq!(run_id(1_762_869_845, Some(3)), "20251111-140405-v3");
    assert_eq!(run_id(951_782_400, None), "20000229-000000");
}

#[test]
fn the_report_sorts_by_severity_and_carries_the_text() {
    let report = Report::of(&outcome(), "r1", 7);
    assert_eq!(report.findings[0].rule, "images.alt_missing");
    assert_eq!(report.summary.error, 1);
    assert_eq!(report.summary.warning, 1);
    assert_eq!(report.images.ok, 0);
    assert_eq!(report.images.total, 1);
    assert!(
        !report.findings[1].text.contains('{'),
        "{}",
        report.findings[1].text
    );
}

#[test]
fn both_files_are_written_and_the_html_is_escaped() {
    let mut host = MemoryHost::new().grant(Capability::OutWrite);
    let report = Report::of(&outcome(), "20251111-140405", 7);
    let paths = write(&mut host, &report).expect("written");
    assert_eq!(
        paths,
        [
            ".rbrowser/out/seo/20251111-140405/report.json",
            ".rbrowser/out/seo/20251111-140405/report.html"
        ]
    );
    let json: serde_json::Value =
        serde_json::from_slice(&host.out["20251111-140405/report.json"]).expect("json");
    assert_eq!(json["version"], 1);
    assert_eq!(json["scope"], "page");
    assert_eq!(json["findings"][0]["severity"], "error");
    let html = String::from_utf8(host.out["20251111-140405/report.html"].clone()).expect("utf-8");
    assert!(!html.contains("<script"));
    assert!(html.contains("&lt;Shop&gt;"));
    assert!(html.contains("default-src 'none'"));
}

#[test]
fn without_out_write_the_report_is_refused() {
    let mut host = MemoryHost::new();
    let report = Report::of(&outcome(), "r", 0);
    assert!(write(&mut host, &report).is_err());
}
