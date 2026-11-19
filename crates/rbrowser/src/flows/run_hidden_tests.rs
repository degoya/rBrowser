//! RB-090-84, the owner's decision: a run plays in the views shown; the views a layout hides are
//! skipped with the reason, and the breakpoints the flow names that no view shows are named.

use std::collections::BTreeSet;

use serde_json::json;
use tokio::sync::{mpsc, watch};

use super::tests::{Fake, Project, passed, played};
use super::*;
use crate::cdp::lock;
use crate::flows::plan::HIDDEN_REASON;

#[tokio::test]
async fn a_hidden_view_is_skipped_and_the_breakpoints_no_view_shows_are_named() {
    let project = Project::new("await page.goto(env);\n");
    project.trust();
    let fake = Fake::default();
    *lock(&fake.answer) = Some(Ok(json!({ "views": [passed(1)] })));
    let mut setup = project.setup();
    setup.hidden = BTreeSet::from([ViewId(2)]);
    let mut asked = played();
    if let Some(inline) = asked.inline.as_mut() {
        inline.views = vec!["mobile".to_owned(), "tablet".to_owned(), "w1600".to_owned()];
    }
    let (_cancel, cancelled) = watch::channel(false);
    let (progress, _received) = mpsc::unbounded_channel();
    let result = execute(&setup, &asked, &fake, cancelled, progress)
        .await
        .expect("runs");
    // Only the view shown runs.
    assert_eq!(lock(&fake.opened)[0].0, [1]);
    let tablet = result
        .views
        .iter()
        .find(|view| view.view_id == 2)
        .expect("the hidden view");
    assert_eq!(tablet.status, ViewStatus::Skipped);
    assert_eq!(tablet.reason.as_deref(), Some(HIDDEN_REASON));
    assert_eq!(result.skipped_breakpoints, ["w1600"]);
    assert!(result.passed);
}

#[tokio::test]
async fn views_asked_for_name_no_missing_breakpoints() {
    let project = Project::new("// @views mobile, watch\nawait page.goto(env);\n");
    project.trust();
    let fake = Fake::default();
    *lock(&fake.answer) = Some(Ok(json!({ "views": [passed(1)] })));
    let (_cancel, cancelled) = watch::channel(false);
    let (progress, _received) = mpsc::unbounded_channel();
    let mut asked = played();
    asked.inline = None;
    asked.name = "login".to_owned();
    asked.views = Some(vec![ViewId(1)]);
    let result = execute(&project.setup(), &asked, &fake, cancelled, progress)
        .await
        .expect("runs");
    assert!(result.skipped_breakpoints.is_empty());
}

#[tokio::test]
async fn a_view_asked_for_plays_even_when_hidden() {
    // `flows.run { views }`, MCP `target: viewId`: the person named it.
    let project = Project::new("await page.goto(env);\n");
    project.trust();
    let fake = Fake::default();
    *lock(&fake.answer) = Some(Ok(json!({ "views": [passed(2)] })));
    let mut setup = project.setup();
    setup.hidden = BTreeSet::from([ViewId(2)]);
    let mut asked = played();
    asked.views = Some(vec![ViewId(2)]);
    let (_cancel, cancelled) = watch::channel(false);
    let (progress, _received) = mpsc::unbounded_channel();
    let result = execute(&setup, &asked, &fake, cancelled, progress)
        .await
        .expect("runs");
    assert_eq!(lock(&fake.opened)[0].0, [2]);
    assert!(result.passed);
}

#[tokio::test]
async fn a_run_whose_every_view_is_hidden_fails_and_plays_nothing() {
    let project = Project::new("await page.goto(env);\n");
    project.trust();
    let fake = Fake::default();
    let mut setup = project.setup();
    setup.hidden = BTreeSet::from([ViewId(1)]);
    let (_cancel, cancelled) = watch::channel(false);
    let (progress, _received) = mpsc::unbounded_channel();
    let outcome = execute(&setup, &played(), &fake, cancelled, progress).await;
    let Err(FlowError::NoViews(why)) = outcome else {
        panic!("no views: {outcome:?}");
    };
    assert!(why.contains("hidden in the layout"), "{why}");
    assert!(lock(&fake.opened).is_empty());
    assert!(lock(&fake.calls).is_empty());
}

#[test]
fn a_run_of_skipped_views_names_why_nothing_plays() {
    use crate::flows::plan::{FIREFOX_REASON, nothing_to_play, skipped};
    let view = rb_mcp::testing::view(3, "desktop", "https://a.test/");
    assert_eq!(
        nothing_to_play(&[
            skipped(&view, FIREFOX_REASON),
            skipped(&view, FIREFOX_REASON)
        ]),
        format!("no view of the run can play: {FIREFOX_REASON}")
    );
}
