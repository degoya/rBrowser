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
    run_queue(
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
}
