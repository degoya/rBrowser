use serde_json::json;

use super::*;
use crate::fake::FakeHost;

const URL: &str = "http://localhost:8080/fonts/index.html";

fn host() -> FakeHost {
    let mut host = FakeHost::default();
    host.view(1, "Desktop", 1280, URL)
        .view(2, "Mobile", 375, URL);
    host.pages
        .insert(1, include_str!("../tests/fixtures/wide.json").to_owned());
    host.pages
        .insert(2, include_str!("../tests/fixtures/narrow.json").to_owned());
    host
}

#[test]
fn list_answers_the_inventory_with_the_findings() {
    let mut host = host();
    let answer = list(&mut host, &json!({ "target": "all", "rendered": false })).expect("list");
    assert_eq!(answer["views"].as_array().map(Vec::len), Some(2));
    let families: Vec<&str> = answer["families"]
        .as_array()
        .expect("families")
        .iter()
        .filter_map(|f| f["family"].as_str())
        .collect();
    assert!(families.starts_with(&[
        "Inter",
        "Roboto",
        "Legacy Serif",
        "Unused Display",
        "Inter ASCII"
    ]));
    assert!(answer["roles"].as_array().is_some_and(|r| !r.is_empty()));
    let rules: Vec<&str> = answer["findings"]
        .as_array()
        .expect("findings")
        .iter()
        .filter_map(|f| f["rule"].as_str())
        .collect();
    assert!(rules.contains(&"external-host") && rules.contains(&"fluid-not-scaled"));
    assert!(host.cdp_sent.is_empty());
}

#[test]
fn list_fits_its_budget() {
    let mut host = host();
    let answer = list(
        &mut host,
        &json!({ "budget": MIN_BUDGET, "rendered": false }),
    )
    .expect("list");
    assert!(answer.to_string().len() <= MIN_BUDGET);
    assert_eq!(answer["cut"], true);
    for bad in [
        json!({ "budget": 10 }),
        json!({ "budget": "x" }),
        json!({ "rendered": "yes" }),
    ] {
        assert_eq!(
            list(&mut host, &bad).expect_err("bad").code(),
            Some(codes::INVALID)
        );
    }
}

#[test]
fn check_answers_the_findings_with_a_count_per_rule() {
    let mut host = host();
    let answer = check(&mut host, &json!({ "target": 2 })).expect("check");
    assert_eq!(answer["summary"]["input-too-small"], 1);
    assert_eq!(answer["summary"]["external-host"], 1);
    assert!(
        answer["findings"]
            .as_array()
            .is_some_and(|f| f.iter().all(|f| f["view"] == 2))
    );
    assert_eq!(
        answer["findings"][0]["message"],
        "plugin.fonts.findings.external-host"
    );
}

#[test]
fn mark_names_a_family_in_every_view_or_clears() {
    let mut host = host();
    host.marks.insert(1, json!({ "count": 3 }));
    let answer = mark(&mut host, &json!({ "family": "Inter", "weight": 600 })).expect("mark");
    assert_eq!(
        answer["marked"],
        json!([{ "view": 1, "count": 3 }, { "view": 2, "count": 0 }])
    );
    let cleared = mark(&mut host, &json!({ "clear": true, "target": 1 })).expect("clear");
    assert_eq!(cleared["cleared"], true);
    for bad in [
        json!({}),
        json!({ "family": "" }),
        json!({ "family": "x".repeat(MAX_FAMILY_CHARS + 1) }),
        json!({ "family": "a\nb" }),
        json!({ "family": "Inter", "weight": 0 }),
        json!({ "family": "Inter", "style": "bold" }),
    ] {
        assert_eq!(
            mark(&mut host, &bad).expect_err("bad").code(),
            Some(codes::INVALID)
        );
    }
}

#[test]
fn export_writes_json_and_csv() {
    let mut host = host();
    host.now = 1_700_000_000;
    let answer = export(&mut host, &json!({ "rendered": false })).expect("export");
    assert_eq!(
        answer["files"],
        json!([
            ".rbrowser/out/fonts/fonts.json",
            ".rbrowser/out/fonts/fonts.csv"
        ])
    );
    let names: Vec<&str> = host.written.iter().map(|(path, _)| path.as_str()).collect();
    assert_eq!(names, ["fonts.json", "fonts.csv"]);
    let json_text = String::from_utf8(host.written[0].1.clone()).expect("utf-8");
    let value: Value = serde_json::from_str(&json_text).expect("json");
    assert_eq!(value["exportedAt"], 1_700_000_000);
    let csv = String::from_utf8(host.written[1].1.clone()).expect("utf-8");
    assert!(csv.starts_with("view,family,provider"));
    assert!(csv.contains("\r\n1,Inter,own,localhost,true,woff2,false,2,34.5,8,400 600,"));
}

#[test]
fn the_command_reports_the_findings_run() {
    let mut host = host();
    let answer = check_command(&mut host).expect("command");
    assert_eq!(
        answer["findings"].as_u64(),
        Some(host.findings.len() as u64)
    );
    assert!(!host.findings.is_empty());
}

#[test]
fn arguments_must_be_an_object() {
    assert_eq!(parse("").expect("empty"), json!({}));
    assert_eq!(parse("[]").expect_err("array").code(), Some(codes::INVALID));
}
