use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use super::*;

const VIEW: ViewId = ViewId(1);

#[test]
fn a_hundred_requests_while_one_apply_runs_make_one_more_round() {
    let queue = ApplyQueue::default();
    assert!(
        queue.request(VIEW, false),
        "the first request starts a worker"
    );
    assert_eq!(queue.next(VIEW), Some(false));
    for _ in 0..100 {
        assert!(!queue.request(VIEW, false), "the running worker takes them");
    }
    assert_eq!(queue.next(VIEW), Some(false), "one round for all of them");
    assert_eq!(queue.next(VIEW), None, "then the worker stops");
    assert!(
        queue.request(VIEW, false),
        "a later request starts a new one"
    );
}

#[test]
fn a_forced_request_is_not_lost_in_the_coalescing() {
    let queue = ApplyQueue::default();
    assert!(queue.request(VIEW, false));
    assert_eq!(queue.next(VIEW), Some(false));
    assert!(!queue.request(VIEW, true));
    assert!(!queue.request(VIEW, false));
    assert_eq!(queue.next(VIEW), Some(true));
    assert_eq!(queue.next(VIEW), None);
}

#[test]
fn views_are_coalesced_each_on_their_own() {
    let queue = ApplyQueue::default();
    assert!(queue.request(VIEW, false));
    assert!(
        queue.request(ViewId(2), true),
        "another view has its own worker"
    );
    assert_eq!(queue.next(ViewId(2)), Some(true));
    assert_eq!(queue.next(ViewId(2)), None);
    assert_eq!(queue.next(VIEW), Some(false));
}

/// A fake apply that reads the state when it runs, as `apply_metrics` does: a hundred bounds
/// while the first apply waits end in two applies, the second of the last size.
#[tokio::test]
async fn the_last_state_wins_with_few_applies() {
    let queue = Arc::new(ApplyQueue::default());
    let size = Arc::new(AtomicU32::new(0));
    let applied = Arc::new(std::sync::Mutex::new(Vec::new()));
    let (release, gate) = tokio::sync::oneshot::channel::<()>();
    let (started, running) = tokio::sync::oneshot::channel::<()>();
    let worker = {
        let (queue, size, applied) = (queue.clone(), size.clone(), applied.clone());
        size.store(1, Ordering::SeqCst);
        assert!(queue.request(VIEW, false));
        tokio::spawn(async move {
            let mut first = Some((started, gate));
            while queue.next(VIEW).is_some() {
                let seen = size.load(Ordering::SeqCst);
                if let Some((started, gate)) = first.take() {
                    let _ = started.send(());
                    let _ = gate.await;
                }
                applied.lock().expect("not poisoned").push(seen);
            }
        })
    };
    running.await.expect("the first apply runs");
    for width in 2..=100 {
        size.store(width, Ordering::SeqCst);
        assert!(!queue.request(VIEW, false));
    }
    release.send(()).expect("the worker waits");
    worker.await.expect("the worker ends");
    let applied = applied.lock().expect("not poisoned").clone();
    assert_eq!(applied.len(), 2, "{applied:?}");
    assert_eq!(applied.last(), Some(&100), "the last size wins");
}

/// RB-099-18: the queue is idle for a view only once its worker took the last round and found
/// nothing more - what the tests wait for before they say no more metrics come.
#[test]
fn a_view_is_idle_only_after_its_worker_stopped() {
    let queue = ApplyQueue::default();
    assert!(queue.idle(VIEW), "no worker yet");
    assert!(queue.request(VIEW, false));
    assert!(!queue.idle(VIEW), "requested");
    assert_eq!(queue.next(VIEW), Some(false));
    assert!(!queue.idle(VIEW), "the round runs");
    assert_eq!(queue.next(VIEW), None);
    assert!(queue.idle(VIEW), "the worker stopped");
    assert!(queue.idle(ViewId(2)), "another view has none");
}
