use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::{Ended, Job, run_jobs};
use crate::checks::progress::Cancel;

fn sleeper(ms: u64, value: u32) -> Job<u32> {
    Box::pin(async move {
        tokio::time::sleep(Duration::from_millis(ms)).await;
        value
    })
}

/// What `run_jobs` told, as `start i` / `end i v` / `cancel i` lines.
async fn told(limit: usize, jobs: Vec<Job<u32>>, cancel: &Cancel) -> Vec<String> {
    let log = Arc::new(Mutex::new(Vec::new()));
    let (starts, ends) = (Arc::clone(&log), Arc::clone(&log));
    run_jobs(
        limit,
        jobs,
        cancel,
        |index| starts.lock().expect("log").push(format!("start {index}")),
        |index, ended| {
            let line = match ended {
                Ended::Answered(value, _) => format!("end {index} {value}"),
                Ended::Cancelled => format!("cancel {index}"),
                Ended::Panicked => format!("panic {index}"),
            };
            ends.lock().expect("log").push(line);
        },
    )
    .await;
    log.lock().expect("log").clone()
}

#[tokio::test(start_paused = true)]
async fn jobs_end_in_the_order_they_finish_not_the_order_given() {
    let cancel = Cancel::default();
    let lines = told(2, vec![sleeper(30, 10), sleeper(10, 11)], &cancel).await;
    assert_eq!(lines, ["start 0", "start 1", "end 1 11", "end 0 10"]);
}

#[tokio::test(start_paused = true)]
async fn a_job_starts_only_when_a_slot_is_free() {
    let cancel = Cancel::default();
    let lines = told(1, vec![sleeper(10, 1), sleeper(10, 2)], &cancel).await;
    assert_eq!(lines, ["start 0", "end 0 1", "start 1", "end 1 2"]);
}

#[tokio::test(start_paused = true)]
async fn a_cancel_drops_the_running_job_and_never_starts_the_waiting_one() {
    let cancel = Arc::new(Cancel::default());
    let stopper = Arc::clone(&cancel);
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(15)).await;
        assert!(stopper.cancel(), "the first cancel cancels");
        assert!(!stopper.cancel(), "a second one changes nothing");
    });
    let lines = told(
        1,
        vec![sleeper(10, 1), sleeper(1000, 2), sleeper(10, 3)],
        &cancel,
    )
    .await;
    assert_eq!(
        lines,
        ["start 0", "end 0 1", "start 1", "cancel 1", "cancel 2"]
    );
}

#[tokio::test(start_paused = true)]
async fn a_run_cancelled_before_it_starts_runs_nothing() {
    let cancel = Cancel::default();
    cancel.cancel();
    let lines = told(4, vec![sleeper(10, 1), sleeper(10, 2)], &cancel).await;
    assert_eq!(lines, ["cancel 0", "cancel 1"]);
}

#[tokio::test(start_paused = true)]
async fn no_jobs_end_at_once() {
    let cancel = Cancel::default();
    assert!(told(4, Vec::new(), &cancel).await.is_empty());
}

#[tokio::test(start_paused = true)]
async fn a_job_that_panics_is_told_as_such_not_as_cancelled() {
    let cancel = Cancel::default();
    let panics: Job<u32> = Box::pin(async {
        tokio::time::sleep(Duration::from_millis(5)).await;
        assert!(cancel_is_never(), "the job panics");
        0
    });
    let lines = told(2, vec![panics, sleeper(10, 2)], &cancel).await;
    assert_eq!(lines, ["start 0", "start 1", "panic 0", "end 1 2"]);
}

fn cancel_is_never() -> bool {
    false
}
