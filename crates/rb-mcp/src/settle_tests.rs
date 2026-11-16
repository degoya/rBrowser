use std::time::Duration;

use rb_engine::ViewId;
use serde_json::json;

use super::settle;
use crate::backend::Backend as _;
use crate::testing::FakeBackend;

/// The runtime's clock is paused: it moves only while something sleeps, so "at once" is a
/// clock that did not move - no poll was waited for.
#[tokio::test(start_paused = true)]
async fn a_changed_url_that_finished_loading_settles_at_once() {
    let fake = FakeBackend::default();
    let before = fake.views();
    fake.run_command(
        "view.navigate",
        json!({ "url": "https://b.test/", "target": "all" }),
    )
    .expect("navigated");
    let started = tokio::time::Instant::now();
    let answer = settle(
        &fake,
        &[ViewId(1), ViewId(2)],
        &before,
        Duration::from_secs(5),
    )
    .await;
    assert!(answer.settled);
    assert_eq!(started.elapsed(), Duration::ZERO, "no poll was waited for");
    let urls: Vec<&str> = answer.views.iter().map(|view| view.url.as_str()).collect();
    assert_eq!(urls, vec!["https://b.test/", "https://b.test/"]);
}

#[tokio::test]
async fn a_view_without_a_load_settles_after_the_grace() {
    let fake = FakeBackend::default();
    let before = fake.views();
    let answer = settle(&fake, &[ViewId(1)], &before, Duration::from_millis(300)).await;
    assert!(answer.settled);
    assert_eq!(answer.views.len(), 1);
}

#[tokio::test]
async fn a_view_still_loading_at_the_timeout_is_not_settled() {
    let fake = FakeBackend::default();
    let before = fake.views();
    fake.set_loading(2, true);
    let answer = settle(&fake, &[ViewId(2)], &before, Duration::from_millis(200)).await;
    assert!(!answer.settled);
    assert!(answer.views.first().is_some_and(|view| view.loading));
}

#[tokio::test]
async fn a_closed_view_is_left_out() {
    let fake = FakeBackend::default();
    let before = fake.views();
    let answer = settle(&fake, &[ViewId(9)], &before, Duration::from_millis(100)).await;
    assert!(answer.settled);
    assert!(answer.views.is_empty());
}
