use std::collections::BTreeMap;

use rb_plugin_api::{CheckContext, FailureKind, codes};
use serde_json::json;

use super::*;
use crate::testing::{PAGE, PerView, URL, answer, host, view};

fn run_all() -> CheckContext {
    CheckContext {
        args: "{}".to_owned(),
        ..CheckContext::default()
    }
}

/// The views of the findings with the property each names.
fn views_and_properties(findings: &[Finding]) -> Vec<(Option<u32>, String)> {
    findings
        .iter()
        .map(|finding| {
            let property = finding
                .params
                .iter()
                .find(|(key, _)| key == "property")
                .map(|(_, value)| value.clone())
                .unwrap_or_default();
            (finding.view, property)
        })
        .collect()
}

#[test]
fn the_target_is_the_view_named_else_the_active_one() {
    let mut host = host(PAGE);
    view(&mut host, 1, URL);
    view(&mut host, 2, URL);
    host.active = Some(2);
    assert_eq!(target(&mut host, &json!(1)).expect("view").id, 1);
    assert_eq!(target(&mut host, &json!(null)).expect("view").id, 2);
    assert_eq!(target(&mut host, &json!("all")).expect("view").id, 2);
    host.active = None;
    assert_eq!(target(&mut host, &json!(null)).expect("view").id, 1);
    assert_eq!(
        target(&mut host, &json!(9)).expect_err("gone").code(),
        Some(codes::NOT_FOUND)
    );
    assert_eq!(
        target(&mut host, &json!("phone")).expect_err("bad").code(),
        Some(codes::INVALID)
    );
}

#[test]
fn an_inspection_reads_the_page_and_names_the_misspelled_property() {
    let mut host = host(PAGE);
    view(&mut host, 1, URL);
    let views = host.views.clone();
    let inspected = inspect(&mut host, &views[0], &Settings::default()).expect("inspected");
    assert_eq!(inspected.page.nodes.len(), 3);
    assert_eq!(inspected.vocabulary, "30.1");
    let rules: Vec<(&str, Option<&str>)> = inspected
        .issues
        .iter()
        .map(|issue| (issue.rule, issue.property.as_deref()))
        .collect();
    assert_eq!(rules, [("property.unknown", Some("headlinee"))]);
}

#[test]
fn a_check_run_reads_every_view_and_reports_an_equal_document_in_each() {
    let mut host = host(PAGE);
    view(&mut host, 1, URL);
    view(&mut host, 2, URL);
    let findings = check(&mut host, &run_all()).expect("findings");
    let views: Vec<Option<u32>> = findings.iter().map(|finding| finding.view).collect();
    assert_eq!(views, [Some(1), Some(2)]);
    assert_eq!(host.evaluated.len(), 2);
    assert_eq!(findings[0].rule, "property.unknown");
    assert_eq!(findings[0].url.as_deref(), Some(URL));
    assert_eq!(host.progress, [(1, 2), (2, 2)]);
}

#[test]
fn a_view_that_cannot_be_read_does_not_hide_a_view_with_the_same_url() {
    let mut host = PerView {
        inner: host(PAGE),
        answers: BTreeMap::new(),
    };
    view(&mut host.inner, 1, URL);
    view(&mut host.inner, 2, URL);
    host.inner.views[0].engine = "firefox".to_owned();
    host.answers.insert(
        1,
        Err(Failure::coded(FailureKind::NotFound, codes::NOT_FOUND)),
    );
    let findings = check(&mut host, &run_all()).expect("findings");
    assert_eq!(
        views_and_properties(&findings),
        [(Some(2), "headlinee".to_owned())]
    );
    assert_eq!(host.inner.progress, [(1, 2), (2, 2)]);
}

#[test]
fn a_breakpoint_rendering_other_structured_data_is_checked_on_its_own() {
    let mut host = PerView {
        inner: host(PAGE),
        answers: BTreeMap::new(),
    };
    view(&mut host.inner, 1, URL);
    view(&mut host.inner, 2, URL);
    view(&mut host.inner, 3, URL);
    let narrow = PAGE.replace("\"headlinee\"", "\"headlinez\"");
    host.answers.insert(2, Ok(answer(URL, &narrow)));
    let findings = check(&mut host, &run_all()).expect("findings");
    assert_eq!(
        views_and_properties(&findings),
        [
            (Some(1), "headlinee".to_owned()),
            (Some(2), "headlinez".to_owned()),
            (Some(3), "headlinee".to_owned()),
        ]
    );
    assert_eq!(host.inner.progress, [(1, 3), (2, 3), (3, 3)]);
}

#[test]
fn a_check_run_of_a_gone_view_fails_and_honours_the_rules_asked_for() {
    let mut host = host(PAGE);
    view(&mut host, 1, URL);
    let gone = CheckContext {
        view: Some(5),
        args: "{}".to_owned(),
        ..CheckContext::default()
    };
    assert_eq!(
        check(&mut host, &gone).expect_err("gone").code(),
        Some(codes::NOT_FOUND)
    );
    let narrow = CheckContext {
        view: Some(1),
        rules: vec!["type.unknown".to_owned()],
        args: "{}".to_owned(),
        ..CheckContext::default()
    };
    assert!(check(&mut host, &narrow).expect("findings").is_empty());
}
