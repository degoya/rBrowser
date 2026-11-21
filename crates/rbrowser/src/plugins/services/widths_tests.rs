use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use rb_plugin_types::{FailureKind, codes};

use super::{CLEAR, SET, WidthLeases, lease_method, requested_width, restore_unless_leased};

/// A guard that counts its drops: the sync pause a lease holds.
fn guard(dropped: &Arc<AtomicUsize>) -> super::Guard {
    struct Counted(Arc<AtomicUsize>);
    impl Drop for Counted {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }
    Box::new(Counted(Arc::clone(dropped)))
}

#[test]
fn the_two_metrics_methods_are_the_lease_and_nothing_else() {
    assert!(lease_method(SET));
    assert!(lease_method(CLEAR));
    assert!(!lease_method("Emulation.setEmulatedVisionDeficiency"));
    assert_eq!(requested_width(r#"{"width":720}"#), Some(720));
    assert_eq!(requested_width(r#"{"width":-1}"#), None);
    assert_eq!(requested_width("{}"), None);
    assert_eq!(requested_width("not json"), None);
}

#[test]
fn a_lease_pauses_the_view_once_and_is_held_until_it_ends() {
    let leases = WidthLeases::default();
    let dropped = Arc::new(AtomicUsize::new(0));
    let mut made = 0;
    leases.started("width-sweep");
    for _ in 0..3 {
        leases
            .take("width-sweep", 1, || {
                made += 1;
                guard(&dropped)
            })
            .expect("the plugin's own lease");
    }
    assert_eq!(made, 1, "the view leaves the sync once");
    assert!(leases.end("width-sweep", 1), "a live lease is restored");
    assert_eq!(dropped.load(Ordering::SeqCst), 1, "sync is back");
    assert!(!leases.end("width-sweep", 1), "ended already");
    assert!(!leases.end("width-sweep", 2), "never leased");
}

#[test]
fn an_ended_lease_refuses_the_next_width_until_the_plugins_calls_are_over() {
    let leases = WidthLeases::default();
    let dropped = Arc::new(AtomicUsize::new(0));
    leases.started("width-sweep");
    leases.started("width-sweep");
    leases
        .take("width-sweep", 1, || guard(&dropped))
        .expect("leased");
    // The cancel call ends it while the sweep call runs on.
    assert!(leases.end("width-sweep", 1));
    let refused = leases
        .take("width-sweep", 1, || guard(&dropped))
        .expect_err("cancelled");
    assert_eq!(refused.kind, FailureKind::Cancelled);
    assert_eq!(refused.code(), Some(codes::CANCELLED));
    assert!(
        leases.ended("width-sweep").is_empty(),
        "the sweep still runs"
    );
    assert!(
        leases.ended("width-sweep").is_empty(),
        "nothing live to restore"
    );
    leases.started("width-sweep");
    leases
        .take("width-sweep", 1, || guard(&dropped))
        .expect("a new call leases again");
}

#[test]
fn the_last_call_ending_restores_every_live_lease_of_its_plugin() {
    let leases = WidthLeases::default();
    let dropped = Arc::new(AtomicUsize::new(0));
    leases.started("width-sweep");
    leases.started("other");
    leases
        .take("width-sweep", 1, || guard(&dropped))
        .expect("leased");
    leases
        .take("width-sweep", 3, || guard(&dropped))
        .expect("leased");
    leases.take("other", 2, || guard(&dropped)).expect("leased");
    assert_eq!(leases.ended("width-sweep"), vec![1, 3]);
    assert_eq!(dropped.load(Ordering::SeqCst), 2);
    assert_eq!(leases.ended("other"), vec![2]);
    assert_eq!(dropped.load(Ordering::SeqCst), 3);
    // An end without a start (a span from before the services were bound) changes nothing.
    assert!(leases.ended("width-sweep").is_empty());
}

#[test]
fn another_plugins_view_is_busy() {
    let leases = WidthLeases::default();
    let dropped = Arc::new(AtomicUsize::new(0));
    leases.started("width-sweep");
    leases
        .take("width-sweep", 1, || guard(&dropped))
        .expect("leased");
    let busy = leases
        .take("other", 1, || guard(&dropped))
        .expect_err("busy");
    assert_eq!(busy.code(), Some(codes::VIEW_LEASED));
    assert_eq!(busy.param("view"), Some("1"));
    assert!(!leases.end("other", 1), "nor can it end another's lease");
    assert_eq!(dropped.load(Ordering::SeqCst), 0);
}

#[test]
fn a_stopped_plugin_gives_back_every_view_at_once() {
    let leases = WidthLeases::default();
    let dropped = Arc::new(AtomicUsize::new(0));
    leases.started("width-sweep");
    leases
        .take("width-sweep", 4, || guard(&dropped))
        .expect("leased");
    assert_eq!(leases.stop("width-sweep"), vec![4]);
    assert_eq!(dropped.load(Ordering::SeqCst), 1);
    assert!(leases.stop("width-sweep").is_empty());
}

/// A restore that counts its sends.
fn sender(sent: &Arc<AtomicUsize>) -> impl FnOnce() -> std::future::Ready<()> {
    let sent = Arc::clone(sent);
    move || {
        sent.fetch_add(1, Ordering::SeqCst);
        std::future::ready(())
    }
}

#[tokio::test]
async fn a_late_restore_never_undoes_a_lease_taken_after_the_call_ended() {
    let leases = WidthLeases::default();
    let dropped = Arc::new(AtomicUsize::new(0));
    let sent = Arc::new(AtomicUsize::new(0));
    leases.started("width-sweep");
    leases
        .take("width-sweep", 1, || guard(&dropped))
        .expect("leased");
    assert_eq!(leases.ended("width-sweep"), vec![1], "the call ended");
    // The next sweep call takes the view before the restore spawned at the end runs.
    leases.started("width-sweep");
    leases
        .take("width-sweep", 1, || guard(&dropped))
        .expect("leased again");
    let generation = leases.generation(1);
    assert!(!restore_unless_leased(&leases, 1, generation, sender(&sent)).await);
    assert_eq!(
        sent.load(Ordering::SeqCst),
        0,
        "the new lease keeps its width"
    );
    // Without a lease taken since, the view gets its own width back.
    assert_eq!(leases.ended("width-sweep"), vec![1]);
    let generation = leases.generation(1);
    assert!(restore_unless_leased(&leases, 1, generation, sender(&sent)).await);
    assert_eq!(sent.load(Ordering::SeqCst), 1);
}

/// RB-094-10: a lease taken and ended again before a late restore runs restored the view at its
/// own end; the late restore, of an older generation, sends nothing.
#[tokio::test]
async fn a_restore_of_an_older_lease_generation_sends_nothing() {
    let leases = WidthLeases::default();
    let dropped = Arc::new(AtomicUsize::new(0));
    let sent = Arc::new(AtomicUsize::new(0));
    assert_eq!(leases.generation(1), 0, "never leased");
    leases.started("width-sweep");
    leases
        .take("width-sweep", 1, || guard(&dropped))
        .expect("leased");
    assert_eq!(leases.ended("width-sweep"), vec![1]);
    let late = leases.generation(1);
    leases.started("width-sweep");
    leases
        .take("width-sweep", 1, || guard(&dropped))
        .expect("leased again");
    // A width of the live lease is no new generation.
    leases
        .take("width-sweep", 1, || guard(&dropped))
        .expect("held");
    assert_eq!(leases.generation(1), late + 1);
    assert_eq!(leases.ended("width-sweep"), vec![1]);
    assert!(!restore_unless_leased(&leases, 1, late, sender(&sent)).await);
    assert_eq!(sent.load(Ordering::SeqCst), 0);
    assert!(restore_unless_leased(&leases, 1, late + 1, sender(&sent)).await);
    assert_eq!(sent.load(Ordering::SeqCst), 1);
}

/// RB-094-10: the check and the send of a restore are one step - the view's width waits for the
/// restore's send to end, so it is sent after it and is the one that holds.
#[tokio::test]
async fn a_width_waits_for_the_restore_in_flight() {
    let leases = Arc::new(WidthLeases::default());
    let (entered_tx, entered_rx) = tokio::sync::oneshot::channel::<()>();
    let (release_tx, release_rx) = tokio::sync::oneshot::channel::<()>();
    let generation = leases.generation(1);
    let task = tokio::spawn({
        let leases = Arc::clone(&leases);
        async move {
            let send = move || async move {
                let _ = entered_tx.send(());
                let _ = release_rx.await;
            };
            restore_unless_leased(&leases, 1, generation, send).await
        }
    });
    entered_rx.await.expect("the restore sends");
    assert!(
        leases.gate(1).try_lock().is_err(),
        "a width for the view waits for the restore"
    );
    assert!(leases.gate(2).try_lock().is_ok(), "another view does not");
    release_tx.send(()).expect("released");
    assert!(task.await.expect("joined"));
    assert!(leases.gate(1).try_lock().is_ok());
}
