use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use tokio::sync::oneshot;

use super::*;
use crate::cdp::lock;
use crate::flows::app::Started;

/// A run that has already ended.
fn ended(name: &str) -> Started {
    let (sender, done) = oneshot::channel();
    let _ = sender.send(Err(FlowError::Io(format!("{name} ran"))));
    Started {
        run_id: name.to_owned(),
        done,
    }
}

fn names(list: &[&str]) -> Vec<String> {
    list.iter().map(|name| (*name).to_owned()).collect()
}

#[tokio::test]
async fn every_flow_runs_one_after_the_other() {
    let started = Mutex::new(Vec::new());
    run_queue(
        names(&["a", "b", "c"]),
        || false,
        |name| {
            lock(&started).push(name.to_owned());
            Ok(ended(name))
        },
    )
    .await;
    assert_eq!(*lock(&started), ["a", "b", "c"]);
}

#[tokio::test]
async fn a_cancel_during_a_flow_ends_the_queue() {
    let started = Mutex::new(Vec::new());
    let cancelled = AtomicBool::new(false);
    run_queue(
        names(&["a", "b", "c"]),
        || cancelled.load(Ordering::SeqCst),
        |name| {
            lock(&started).push(name.to_owned());
            // The person cancels while "b" runs.
            if name == "b" {
                cancelled.store(true, Ordering::SeqCst);
            }
            Ok(ended(name))
        },
    )
    .await;
    assert_eq!(*lock(&started), ["a", "b"]);
}

#[tokio::test]
async fn a_flow_that_does_not_start_ends_the_queue() {
    let started = Mutex::new(Vec::new());
    let refused = run_queue(
        names(&["a", "b", "c"]),
        || false,
        |name| {
            lock(&started).push(name.to_owned());
            if name == "b" {
                return Err(FlowError::Busy("other".to_owned()));
            }
            Ok(ended(name))
        },
    )
    .await;
    assert_eq!(*lock(&started), ["a", "b"]);
    // The queue says why it ended and which flows did not run (RB-090-71).
    assert_eq!(
        refused,
        Some(Refused {
            flow: "b".to_owned(),
            error: FlowError::Busy("other".to_owned()),
            not_run: names(&["b", "c"]),
        })
    );
}

#[tokio::test]
async fn a_queue_that_ends_or_is_cancelled_names_no_refusal() {
    let all = run_queue(names(&["a", "b"]), || false, |name| Ok(ended(name))).await;
    assert_eq!(all, None);
    let cancelled = run_queue(names(&["a", "b"]), || true, |name| Ok(ended(name))).await;
    assert_eq!(cancelled, None);
}

#[test]
fn a_refusal_reads_as_a_sentence_with_the_flows_not_run() {
    let refused = Refused {
        flow: "cart".to_owned(),
        error: FlowError::Recording("login".to_owned()),
        not_run: names(&["cart", "search"]),
    };
    let args = refused.notice_args();
    assert_eq!(args[0], ("flow", "cart".to_owned()));
    assert!(args[1].1.contains("recorder.stop"));
    assert_eq!(args[2], ("flows", "cart, search".to_owned()));
}

#[tokio::test]
async fn the_waiting_flows_shrink_as_the_queue_starts_them_and_are_none_after() {
    // The run state names them, so "Run all" does not read as one flow looping (RB-090-71).
    set_waiting(names(&["b", "c"]));
    let seen = Mutex::new(Vec::new());
    run_queue(
        names(&["b", "c"]),
        || false,
        |name| {
            lock(&seen).push((name.to_owned(), waiting()));
            Ok(ended(name))
        },
    )
    .await;
    assert_eq!(
        *lock(&seen),
        [
            ("b".to_owned(), names(&["c"])),
            ("c".to_owned(), Vec::<String>::new())
        ]
    );
    assert!(waiting().is_empty());
}

#[test]
fn a_cancel_empties_the_waiting_flows() {
    set_waiting(names(&["b", "c"]));
    note_cancel();
    assert!(waiting().is_empty());
}

#[tokio::test]
async fn a_queue_that_stops_early_leaves_no_flow_waiting() {
    set_waiting(names(&["b", "c"]));
    run_queue(
        names(&["b", "c"]),
        || false,
        |_| Err(FlowError::Busy("other".to_owned())),
    )
    .await;
    assert!(waiting().is_empty());
}
