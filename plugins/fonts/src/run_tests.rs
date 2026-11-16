use serde_json::json;

use super::*;
use crate::fake::FakeHost;

const URL: &str = "http://localhost:8080/fonts/index.html";
const WIDE: &str = include_str!("../tests/fixtures/wide.json");
const NARROW: &str = include_str!("../tests/fixtures/narrow.json");

fn host() -> FakeHost {
    let mut host = FakeHost::default();
    host.view(1, "Desktop", 1280, URL)
        .view(2, "Mobile", 375, URL);
    host.pages.insert(1, WIDE.to_owned());
    host.pages.insert(2, NARROW.to_owned());
    host
}

fn ctx(view: Option<ViewId>, rules: &[&str]) -> CheckContext {
    CheckContext {
        view,
        url: None,
        rules: rules.iter().map(|rule| (*rule).to_owned()).collect(),
        args: String::new(),
    }
}

#[test]
fn targets_all_one_or_refused() {
    let mut host = host();
    assert_eq!(targets(&mut host, &Value::Null).expect("all").len(), 2);
    assert_eq!(targets(&mut host, &json!("all")).expect("all").len(), 2);
    assert_eq!(targets(&mut host, &json!(2)).expect("one")[0].id, 2);
    let missing = targets(&mut host, &json!(9)).expect_err("missing");
    assert_eq!(missing.code(), Some(codes::NOT_FOUND));
    let bad = targets(&mut host, &json!("mobile")).expect_err("bad");
    assert_eq!(bad.code(), Some(codes::INVALID));
}

#[test]
fn samples_take_the_first_and_every_one_beyond_ascii_without_generic_families() {
    let fonts = crate::fake::fixture("narrow");
    let chosen = samples(&fonts);
    let families: Vec<&str> = chosen
        .iter()
        .map(|(usage, _)| fonts.usages[*usage].family.as_str())
        .collect();
    assert_eq!(
        families,
        ["Inter", "Inter", "Roboto", "Legacy Serif", "Inter ASCII"]
    );
}

#[test]
fn the_rendered_fonts_are_read_per_sample_and_judged() {
    let mut host = host();
    host.rendered("#q", 0, &[]);
    host.rendered(
        "html > body > main > p.subset",
        40,
        &[("Inter", true, 30), ("Arial", false, 5)],
    );
    let view = host.views[1].clone();
    let fonts = collect_view(&mut host, &view, &Settings::default(), true).expect("collected");
    let results = fonts.rendered.results().expect("read");
    let subset = results
        .iter()
        .find(|r| r.family == "Inter ASCII")
        .expect("subset");
    assert_eq!(
        subset.verdict,
        Verdict::Glyphs {
            fallback: "Arial".to_owned(),
            glyphs: 5
        }
    );
    assert_eq!(subset.non_ascii.chars().count(), 5);
    assert!(
        results
            .iter()
            .filter(|r| r.family != "Inter ASCII")
            .all(|r| r.verdict == Verdict::Unknown)
    );
}

#[test]
fn a_check_run_of_one_view_gives_its_findings_and_the_lead_the_breakpoints() {
    let mut host = host();
    let mobile = check(&mut host, &ctx(Some(2), &[])).expect("mobile");
    assert!(mobile.iter().all(|f| f.view == Some(2)));
    assert!(mobile.iter().any(|f| f.rule == "input-too-small"));
    assert!(!mobile.iter().any(|f| f.rule == "fluid-not-scaled"));
    let desktop = check(&mut host, &ctx(Some(1), &[])).expect("desktop");
    let fluid: Vec<_> = desktop
        .iter()
        .filter(|f| f.rule == "fluid-not-scaled")
        .collect();
    assert_eq!(fluid.len(), 1);
    assert_eq!(fluid[0].path.as_deref(), Some("html > body > header > h1"));
}

#[test]
fn rules_switched_off_spare_the_cdp_part() {
    let mut host = host();
    let found = check(&mut host, &ctx(None, &["text-too-small"])).expect("run");
    assert!(found.iter().all(|f| f.rule == "text-too-small"));
    assert_eq!(found.len(), 2);
    assert!(host.cdp_sent.is_empty());
}

#[test]
fn a_firefox_view_named_alone_is_skipped_for_its_engine_and_left_out_of_all() {
    let mut host = host();
    host.views[1].engine = "firefox".to_owned();
    let failure = check(&mut host, &ctx(Some(2), &[])).expect_err("skipped");
    assert_eq!(failure.unserved_engine_in(2), Some("firefox"));
    let all = check(&mut host, &ctx(None, &[])).expect("run");
    assert!(all.iter().all(|f| f.view == Some(1)));
}

#[test]
fn a_view_that_went_away_fails_the_run_that_names_it() {
    let mut host = host();
    host.pages.remove(&2);
    let failure = check(&mut host, &ctx(Some(2), &[])).expect_err("gone");
    assert_eq!(failure.code(), Some(codes::NOT_FOUND));
    let views = host.views.clone();
    let collected = collect(&mut host, &views, &Settings::default(), false);
    assert_eq!(collected.views.len(), 1);
    assert_eq!(collected.failures[0].1, codes::NOT_FOUND);
}
