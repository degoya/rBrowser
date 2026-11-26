use futures_util::FutureExt;

use super::*;

#[tokio::test]
async fn one_views_apply_does_not_wait_for_another_views() {
    let locks = ApplyLocks::default();
    let _first = locks.view(ViewId(1)).await;
    assert!(
        locks.view(ViewId(2)).now_or_never().is_some(),
        "view 2 waited for view 1"
    );
}

#[tokio::test]
async fn two_applies_of_one_view_take_turns() {
    let locks = ApplyLocks::default();
    let first = locks.view(ViewId(1)).await;
    let mut second = Box::pin(locks.view(ViewId(1)));
    assert!(
        (&mut second).now_or_never().is_none(),
        "the second apply of the view ran beside the first"
    );
    drop(first);
    assert!(second.now_or_never().is_some(), "the second one goes next");
}

#[tokio::test]
async fn the_grants_of_one_context_take_turns_and_other_contexts_do_not_wait() {
    let locks = ApplyLocks::default();
    let first = locks.context("default").await;
    assert!(
        locks.context("staging").now_or_never().is_some(),
        "another context waited"
    );
    let mut second = Box::pin(locks.context("default"));
    assert!(
        (&mut second).now_or_never().is_none(),
        "two grant plans of one context ran at once"
    );
    drop(first);
    assert!(second.now_or_never().is_some());
}

#[tokio::test]
async fn a_view_and_its_context_are_separate_locks() {
    let locks = ApplyLocks::default();
    let _view = locks.view(ViewId(1)).await;
    assert!(locks.context("default").now_or_never().is_some());
}

#[tokio::test]
async fn locks_nobody_holds_are_dropped() {
    let locks = ApplyLocks::default();
    for view in 1..=8 {
        drop(locks.view(ViewId(view)).await);
    }
    let _held = locks.view(ViewId(9)).await;
    assert_eq!(locks.views.len(), 1, "only the held lock is kept");
}
