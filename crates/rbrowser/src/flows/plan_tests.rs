use std::collections::BTreeSet;

use rb_engine::{EngineKind, ViewId, ViewInfo};
use rb_mcp::testing::view;

use super::*;

fn grid() -> Vec<ViewInfo> {
    let mut firefox = view(3, "desktop", "https://a.test/");
    firefox.engine = EngineKind::Firefox;
    firefox.label = "Desktop Firefox".to_owned();
    let mut webkit = view(4, "mobile", "https://a.test/");
    webkit.engine = EngineKind::Webkit;
    vec![
        view(1, "mobile", "https://a.test/"),
        view(2, "tablet", "https://a.test/"),
        firefox,
        webkit,
        view(5, "desktop", "https://a.test/"),
    ]
}

fn target(id: ViewId) -> Option<String> {
    (id != ViewId(5)).then(|| format!("T{}", id.0))
}

fn ids(planned: &Planned) -> (Vec<u32>, Vec<(u32, String)>) {
    (
        planned.run.iter().map(|run| run.view.id.0).collect(),
        planned
            .skipped
            .iter()
            .map(|view| (view.view_id, view.reason.clone().unwrap_or_default()))
            .collect(),
    )
}

#[test]
fn all_views_run_but_firefox_webkit_and_pageless_ones_are_skipped_with_a_reason() {
    let planned = plan(&grid(), None, &[], target).expect("plans");
    let (run, skipped) = ids(&planned);
    assert_eq!(run, [1, 2]);
    assert_eq!(
        skipped,
        [
            (3, FIREFOX_REASON.to_owned()),
            (4, WEBKIT_REASON.to_owned()),
            (5, NO_PAGE_REASON.to_owned()),
        ]
    );
    assert_eq!(planned.run[0].target, "T1");
    assert_eq!(planned.skipped[0].engine, "firefox");
    assert_eq!(planned.skipped[0].status, ViewStatus::Skipped);
}

#[test]
fn asked_views_win_over_the_head_and_an_unknown_one_is_refused() {
    let asked = [ViewId(2)];
    let planned = plan(&grid(), Some(&asked), &["mobile".to_owned()], target).expect("plans");
    assert_eq!(ids(&planned).0, [2]);
    assert!(matches!(
        plan(&grid(), Some(&[ViewId(9)]), &[], target),
        Err(FlowError::NoViews(_))
    ));
}

#[test]
fn the_head_picks_views_by_breakpoint_or_label() {
    let planned = plan(&grid(), None, &["MOBILE".to_owned()], target).expect("plans");
    assert_eq!(ids(&planned).0, [1]);
    assert_eq!(
        ids(&planned).1.len(),
        1,
        "the WebKit view on mobile is skipped"
    );
    let by_label = plan(&grid(), None, &["desktop firefox".to_owned()], target).expect("plans");
    assert_eq!(ids(&by_label).1[0].0, 3);
    assert!(matches!(
        plan(&grid(), None, &["watch".to_owned()], target),
        Err(FlowError::NoViews(_))
    ));
}

#[test]
fn a_view_the_layout_hides_is_skipped_with_the_reason() {
    // RB-090-84, the owner's decision: the run plays in the views shown.
    let planned = plan(&grid(), None, &[], target).expect("plans");
    let planned = without_hidden(planned, &BTreeSet::from([ViewId(2)]));
    let (run, skipped) = ids(&planned);
    assert_eq!(run, [1]);
    assert!(
        skipped.contains(&(2, HIDDEN_REASON.to_owned())),
        "{skipped:?}"
    );
    assert_eq!(planned.skipped.len(), 4);
}

#[test]
fn the_breakpoints_a_flow_names_that_no_view_shows_are_named() {
    // Focus mode shows one breakpoint: the recording's others are no view at all.
    let views = vec![view(1, "w1600", "https://a.test/")];
    let names = ["w1600".to_owned(), "mobile".to_owned(), "Tablet".to_owned()];
    assert_eq!(missing_breakpoints(&views, &names), ["mobile", "Tablet"]);
    assert!(missing_breakpoints(&views, &[]).is_empty());
}
