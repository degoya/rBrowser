use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::*;

/// Waits (bounded) until `done` jobs have run.
async fn until_done(seen: &Arc<Mutex<Vec<u32>>>, done: usize) {
    tokio::time::timeout(Duration::from_secs(10), async {
        while lock(seen).len() < done {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("every job ran");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_steps_run_one_after_the_other_in_the_order_they_came() {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let busy = Arc::new(Mutex::new(false));
    // A panic in a job is caught by the lane, so an overlap is counted, not asserted there.
    let overlaps = Arc::new(Mutex::new(0_u32));
    for id in 0..30_u32 {
        let (seen, busy, overlaps) = (seen.clone(), busy.clone(), overlaps.clone());
        run_in_order(&Handle::current(), move || {
            if std::mem::replace(&mut *lock(&busy), true) {
                *lock(&overlaps) += 1;
            }
            // A slow first step (the analyser reading a site) does not let a later one pass it.
            if id == 0 {
                std::thread::sleep(Duration::from_millis(50));
            }
            lock(&seen).push(id);
            *lock(&busy) = false;
        });
    }
    until_done(&seen, 30).await;
    assert_eq!(*lock(&seen), (0..30).collect::<Vec<_>>());
    assert_eq!(*lock(&overlaps), 0, "never two at once");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_step_that_panics_does_not_stop_the_next() {
    let seen = Arc::new(Mutex::new(Vec::new()));
    run_in_order(&Handle::current(), || panic!("a broken step"));
    let after = seen.clone();
    run_in_order(&Handle::current(), move || lock(&after).push(1));
    until_done(&seen, 1).await;
}
