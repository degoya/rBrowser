use rb_plugin_api::{CheckContext, codes};
use serde_json::{Value, json};

use super::*;
use crate::fake::FakeHost;

const MOBILE: &str = include_str!("../tests/fixtures/mobile.json");
const DESKTOP: &str = include_str!("../tests/fixtures/desktop.json");

fn json(text: &str) -> Value {
    serde_json::from_str(text).expect("fixture")
}

/// A mobile view at DPR 3, a desktop view at DPR 1 and a Firefox view.
fn host() -> FakeHost {
    let mut host = FakeHost::default();
    host.view(1, "Mobile", "https://example.test/")
        .shaped("chromium", 390, 3.0)
        .page(1, &json(MOBILE));
    host.view(2, "Desktop", "https://example.test/")
        .shaped("chromium", 1440, 1.0)
        .page(2, &json(DESKTOP));
    host.view(3, "Firefox", "https://example.test/")
        .shaped("firefox", 1280, 1.0);
    host
}

fn ctx(view: Option<u32>, rules: &[&str]) -> CheckContext {
    CheckContext {
        view,
        url: None,
        rules: rules.iter().map(|rule| (*rule).to_owned()).collect(),
        args: "{}".to_owned(),
    }
}

#[test]
fn targets_are_every_view_one_view_or_refused() {
    let mut host = host();
    let ids = |views: Vec<ViewInfo>| views.into_iter().map(|view| view.id).collect::<Vec<_>>();
    assert_eq!(
        ids(targets(&mut host, &Value::Null).expect("all")),
        [1, 2, 3]
    );
    assert_eq!(
        ids(targets(&mut host, &json!("all")).expect("all")),
        [1, 2, 3]
    );
    assert_eq!(ids(targets(&mut host, &json!(2)).expect("one")), [2]);
    let gone = targets(&mut host, &json!(9)).expect_err("gone");
    assert_eq!(gone.code(), Some(codes::NOT_FOUND));
    for bad in [json!(-1), json!("one"), json!([1])] {
        let failure = targets(&mut host, &bad).expect_err("bad");
        assert_eq!(failure.code(), Some(codes::INVALID), "{bad}");
    }
}

#[test]
fn each_view_is_measured_at_its_own_density() {
    let mut host = host();
    let views = host.views.clone();
    let measured = measure(&mut host, views, &Settings::default());
    assert_eq!(measured.len(), 3);
    assert!(host.evaluated[0].1.contains("\"dpr\":3.0"));
    assert!(host.evaluated[1].1.contains("\"dpr\":1.0"));
    assert_eq!(host.progress, [(1, 3), (2, 3), (3, 3)]);
    let failure = measured[2].page.as_ref().expect_err("firefox");
    assert_eq!(failure.code(), Some(codes::NOT_AVAILABLE));
}

#[test]
fn a_run_over_every_view_finds_the_four_problems_per_breakpoint_and_skips_firefox() {
    let mut host = host();
    let found = check(&mut host, &ctx(None, &[])).expect("run");
    let per_view = |view: u32| {
        found
            .iter()
            .filter(|finding| finding.view == Some(view))
            .map(|finding| finding.rule.as_str())
            .collect::<Vec<_>>()
    };
    let four = ["lcp", "oversized", "undersized", "sizes-missing"];
    assert_eq!(per_view(1), four);
    assert_eq!(per_view(2), four);
    assert!(per_view(3).is_empty());
    let densities: Vec<(Option<u32>, String)> = found
        .iter()
        .map(|finding| {
            let dpr = finding
                .params
                .iter()
                .find(|(key, _)| key == "dpr")
                .map(|(_, value)| value.clone())
                .unwrap_or_default();
            (finding.view, dpr)
        })
        .collect();
    assert!(densities.iter().all(|(view, dpr)| {
        (*view == Some(1) && dpr == "3") || (*view == Some(2) && dpr == "1")
    }));
}

#[test]
fn a_run_of_one_view_takes_that_view_and_its_failure() {
    let mut host = host();
    let found = check(&mut host, &ctx(Some(2), &["oversized"])).expect("run");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].view, Some(2));
    let failure = check(&mut host, &ctx(Some(3), &[])).expect_err("firefox");
    assert_eq!(failure.code(), Some(codes::NOT_AVAILABLE));
    let gone = check(&mut host, &ctx(Some(9), &[])).expect_err("gone");
    assert_eq!(gone.code(), Some(codes::NOT_FOUND));
}
