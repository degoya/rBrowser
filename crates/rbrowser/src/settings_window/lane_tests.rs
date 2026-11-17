use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread::ThreadId;
use std::time::Duration;

use tokio::runtime::Handle;

use super::*;

type Runs = Arc<Mutex<Vec<(usize, ThreadId)>>>;

fn record(runs: &Runs, index: usize) {
    lock(runs).push((index, std::thread::current().id()));
}

#[tokio::test(flavor = "multi_thread")]
async fn jobs_run_off_the_caller_in_the_order_given() {
    let runs: Runs = Arc::default();
    let (tx, rx) = mpsc::channel();
    let caller = std::thread::current().id();
    for index in 0..3 {
        let (runs, tx) = (runs.clone(), tx.clone());
        run_in_order(&Handle::current(), move || {
            if index == 0 {
                // The first takes a while: the others wait for it instead of overtaking.
                std::thread::sleep(Duration::from_millis(30));
            }
            record(&runs, index);
            if index == 2 {
                let _ = tx.send(());
            }
        });
    }
    rx.recv_timeout(Duration::from_secs(5))
        .expect("the last job ran");
    let runs = lock(&runs).clone();
    assert_eq!(
        runs.iter().map(|(index, _)| *index).collect::<Vec<_>>(),
        vec![0, 1, 2]
    );
    assert!(runs.iter().all(|(_, thread)| *thread != caller));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_job_that_panics_does_not_block_the_next() {
    let (tx, rx) = mpsc::channel();
    run_in_order(&Handle::current(), || panic!("a job that fails"));
    run_in_order(&Handle::current(), move || {
        let _ = tx.send(());
    });
    rx.recv_timeout(Duration::from_secs(5))
        .expect("the next job ran");
}

/// The shell's query is answered even when its job panics: an internal failure, not the IPC
/// timeout.
#[tokio::test(flavor = "multi_thread")]
async fn a_job_that_panics_still_answers_with_an_internal_failure() {
    let (tx, rx) = mpsc::channel();
    reply_in_order(
        &Handle::current(),
        || -> u8 { panic!("a job that fails") },
        move |answer| {
            let _ = tx.send(answer);
        },
    );
    let answer = rx.recv_timeout(Duration::from_secs(5)).expect("answered");
    assert!(
        matches!(answer, Err((IpcErrorCode::Internal, _))),
        "{answer:?}"
    );
}

/// A runtime that shuts down drops the drainer unrun: the lane is free again and the next
/// runtime runs what was left queued, in order.
#[test]
fn a_refused_drainer_frees_the_lane_for_the_next_runtime() {
    let gone = tokio::runtime::Builder::new_multi_thread()
        .build()
        .expect("runtime");
    let handle = gone.handle().clone();
    gone.shutdown_background();
    let runs: Runs = Arc::default();
    let first = runs.clone();
    run_in_order(&handle, move || record(&first, 0));
    assert!(
        !lock(lane()).draining,
        "the refused drainer left the lane marked as draining"
    );
    let live = tokio::runtime::Builder::new_multi_thread()
        .build()
        .expect("runtime");
    let (tx, rx) = mpsc::channel();
    let second = runs.clone();
    run_in_order(live.handle(), move || {
        record(&second, 1);
        let _ = tx.send(());
    });
    rx.recv_timeout(Duration::from_secs(5))
        .expect("the queued jobs ran");
    assert_eq!(
        lock(&runs)
            .iter()
            .map(|(index, _)| *index)
            .collect::<Vec<_>>(),
        vec![0, 1]
    );
}
