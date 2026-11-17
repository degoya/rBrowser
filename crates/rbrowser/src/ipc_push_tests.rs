use std::sync::Arc;

use rb_engine::ViewId;
use rb_ipc::{StateSnapshot, ViewMetrics};
use tokio::runtime::Handle;

use super::*;
use crate::cdp::fake::FakeBrowser;
use crate::host::Host;
use crate::host::state::default_config;
use crate::host::ui::recording::RecordingUi;

fn state() -> StateSnapshot {
    let (_fake, router) = FakeBrowser::connect();
    let host = Host::new(Arc::new(RecordingUi::default()), router, Handle::current());
    host.open_config(default_config(), None).expect("opens");
    crate::ipc::snapshot(&host)
}

fn metrics(view: u32) -> ViewMetrics {
    ViewMetrics {
        view: ViewId(view),
        vitals: Vec::new(),
        soft: true,
        resources: None,
        throttle: None,
    }
}

#[tokio::test]
async fn the_same_state_goes_out_once() {
    let state = state();
    let mut push = StatePush::default();
    assert_eq!(
        push.next(state.clone()),
        Some(CoreToShell::State(Box::new(state.clone())))
    );
    assert_eq!(push.next(state.clone()), None, "nothing changed");
    let mut moved = state;
    moved.active_view = Some(ViewId(2));
    assert_eq!(
        push.next(moved.clone()),
        Some(CoreToShell::State(Box::new(moved)))
    );
}

#[tokio::test]
async fn new_metrics_alone_go_out_as_metrics() {
    let state = state();
    let mut push = StatePush::default();
    push.next(state.clone());
    let mut measured = state.clone();
    measured.metrics = vec![metrics(1)];
    assert_eq!(
        push.next(measured.clone()),
        Some(CoreToShell::Metrics {
            metrics: vec![metrics(1)]
        })
    );
    assert_eq!(push.next(measured.clone()), None);
    // Metrics gone are an empty list; another change with them is the whole state.
    assert_eq!(
        push.next(state.clone()),
        Some(CoreToShell::Metrics {
            metrics: Vec::new()
        })
    );
    let mut both = measured;
    both.url = "http://localhost:5173/cart".to_owned();
    assert_eq!(
        push.next(both.clone()),
        Some(CoreToShell::State(Box::new(both)))
    );
}

#[tokio::test]
async fn the_first_state_is_whole_even_with_metrics() {
    let mut state = state();
    state.metrics = vec![metrics(2)];
    let mut push = StatePush::default();
    assert_eq!(
        push.next(state.clone()),
        Some(CoreToShell::State(Box::new(state)))
    );
}

#[tokio::test]
async fn a_forced_push_sends_the_state_sent_last_again() {
    let state = state();
    let mut push = StatePush::default();
    push.next(state.clone());
    push.forget();
    assert_eq!(
        push.next(state.clone()),
        Some(CoreToShell::State(Box::new(state.clone())))
    );
    assert_eq!(push.next(state), None);
}

#[tokio::test]
async fn a_force_is_taken_once_and_only_for_its_window() {
    let (_fake, router) = FakeBrowser::connect();
    let host = Host::new(Arc::new(RecordingUi::default()), router, Handle::current());
    let (_other_fake, other_router) = FakeBrowser::connect();
    let other = Host::new(
        Arc::new(RecordingUi::default()),
        other_router,
        Handle::current(),
    );
    force_next(&host);
    assert!(!take_forced(&other));
    assert!(take_forced(&host));
    assert!(!take_forced(&host), "taken once");
}
