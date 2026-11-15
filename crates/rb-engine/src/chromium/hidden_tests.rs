use std::sync::atomic::Ordering;

use super::*;
use crate::chromium::fake::FakeBackend;

fn spec(breakpoint: &str) -> RenderSpec {
    RenderSpec {
        breakpoint: breakpoint.to_owned(),
        emulation: Emulation::default(),
        session: "default".to_owned(),
    }
}

#[tokio::test]
async fn an_idle_view_is_handed_out_again_for_the_same_spec() {
    let backend = Arc::new(FakeBackend::default());
    let pool = RenderPool::new(backend.clone(), 2);
    let lease = pool.acquire(&spec("mobile")).await.expect("opens");
    let first = lease.id();
    pool.release(lease, true).await;
    let again = pool.acquire(&spec("mobile")).await.expect("reuses");
    assert_eq!(again.id(), first);
    assert_eq!(backend.opened.load(Ordering::Relaxed), 1);
    pool.release(again, true).await;
    assert_eq!(pool.open_views(), 1);
}

#[tokio::test]
async fn a_full_pool_closes_an_idle_view_of_another_spec() {
    let backend = Arc::new(FakeBackend::default());
    let pool = RenderPool::new(backend.clone(), 1);
    let lease = pool.acquire(&spec("mobile")).await.expect("opens");
    let mobile = lease.id();
    pool.release(lease, true).await;
    let lease = pool.acquire(&spec("desktop")).await.expect("opens");
    assert_ne!(lease.id(), mobile);
    assert_eq!(*lock(&backend.closed), [mobile]);
    assert_eq!(pool.open_views(), 1);
    pool.release(lease, true).await;
}

#[tokio::test]
async fn an_unhealthy_view_is_closed_on_release() {
    let backend = Arc::new(FakeBackend::default());
    let pool = RenderPool::new(backend.clone(), 2);
    let lease = pool.acquire(&spec("mobile")).await.expect("opens");
    let id = lease.id();
    pool.release(lease, false).await;
    assert_eq!(*lock(&backend.closed), [id]);
    assert_eq!(pool.open_views(), 0);
}

#[tokio::test]
async fn the_limit_holds_while_views_are_leased() {
    let backend = Arc::new(FakeBackend::default());
    let pool = RenderPool::new(backend.clone(), 2);
    let a = pool.acquire(&spec("a")).await.expect("a");
    let b = pool.acquire(&spec("b")).await.expect("b");
    let third = tokio::time::timeout(Duration::from_millis(50), pool.acquire(&spec("c"))).await;
    assert!(third.is_err(), "a third lease waits");
    pool.release(a, true).await;
    let c = pool.acquire(&spec("c")).await.expect("c after a");
    assert_eq!(pool.open_views(), 2);
    pool.release(b, true).await;
    pool.release(c, true).await;
}

#[tokio::test]
async fn shutdown_closes_idle_and_dropped_views_and_takes_no_new_lease() {
    let backend = Arc::new(FakeBackend::default());
    let pool = RenderPool::new(backend.clone(), 3);
    let kept = pool.acquire(&spec("a")).await.expect("a");
    let dropped = pool.acquire(&spec("b")).await.expect("b");
    let dropped_id = dropped.id();
    drop(dropped);
    pool.release(kept, true).await;
    pool.shutdown().await;
    assert_eq!(lock(&backend.closed).len(), 2);
    assert!(lock(&backend.closed).contains(&dropped_id));
    assert!(lock(&backend.views).is_empty());
    assert!(matches!(
        pool.acquire(&spec("a")).await,
        Err(EngineError::Closed)
    ));
}

#[tokio::test]
async fn a_view_opened_after_its_caller_gave_up_is_closed() {
    let backend = Arc::new(FakeBackend::with_open_close(
        Duration::from_millis(80),
        Duration::ZERO,
    ));
    let pool = RenderPool::new(backend.clone(), 2);
    let gave_up = tokio::time::timeout(Duration::from_millis(10), pool.acquire(&spec("a"))).await;
    assert!(gave_up.is_err(), "the caller stopped waiting");
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(lock(&backend.views).is_empty(), "the late view was closed");
    assert_eq!(lock(&backend.closed).len(), 1);
    assert_eq!(pool.open_views(), 0);
}

#[tokio::test]
async fn a_close_cut_short_by_its_caller_still_happens() {
    let backend = Arc::new(FakeBackend::with_open_close(
        Duration::ZERO,
        Duration::from_millis(50),
    ));
    let pool = RenderPool::new(backend.clone(), 2);
    let lease = pool.acquire(&spec("a")).await.expect("a");
    let cut = tokio::time::timeout(Duration::from_millis(5), pool.release(lease, false)).await;
    assert!(cut.is_err(), "the release was dropped while closing");
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert!(
        lock(&backend.views).is_empty(),
        "the view was closed anyway"
    );
    assert_eq!(pool.open_views(), 0);
}

#[tokio::test]
async fn a_dropped_lease_is_closed_by_the_next_acquire() {
    let backend = Arc::new(FakeBackend::default());
    let pool = RenderPool::new(backend.clone(), 2);
    let dropped = pool.acquire(&spec("a")).await.expect("a");
    let dropped_id = dropped.id();
    drop(dropped);
    let lease = pool.acquire(&spec("b")).await.expect("b");
    assert_eq!(*lock(&backend.closed), [dropped_id]);
    assert_eq!(pool.open_views(), 1, "only the new view is open");
    pool.release(lease, true).await;
}

#[test]
fn the_limit_is_held_between_one_and_the_most() {
    let backend: Arc<dyn RenderBackend> = Arc::new(FakeBackend::default());
    assert_eq!(RenderPool::new(backend.clone(), 0).limit, 1);
    assert_eq!(RenderPool::new(backend, 99).limit, MAX_RENDER_VIEWS);
}
