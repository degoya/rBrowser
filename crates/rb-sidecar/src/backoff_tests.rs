use std::time::{Duration, Instant};

use super::*;

fn policy() -> RestartPolicy {
    RestartPolicy {
        base: Duration::from_millis(500),
        max_delay: Duration::from_secs(2),
        max_crashes: 4,
        stable_after: Duration::from_secs(60),
    }
}

#[test]
fn delays_double_up_to_the_maximum_then_give_up() {
    let now = Instant::now();
    let mut history = CrashHistory::default();
    let delays: Vec<_> = (0..4)
        .map(|_| {
            history.started(now);
            history.crashed(&policy(), now).expect("restarts")
        })
        .collect();
    assert_eq!(
        delays,
        [500, 1000, 2000, 2000].map(Duration::from_millis).to_vec()
    );
    history.started(now);
    assert_eq!(history.crashed(&policy(), now), Err(5));
}

#[test]
fn a_long_run_starts_the_count_again() {
    let now = Instant::now();
    let mut history = CrashHistory::default();
    history.started(now);
    history.crashed(&policy(), now).expect("restarts");
    history.started(now);
    history.crashed(&policy(), now).expect("restarts");
    history.started(now);
    let later = now + Duration::from_secs(61);
    assert_eq!(
        history.crashed(&policy(), later),
        Ok(Duration::from_millis(500))
    );
}

#[test]
fn a_reset_forgets_the_crashes() {
    let now = Instant::now();
    let mut history = CrashHistory::default();
    for _ in 0..5 {
        let _ = history.crashed(&policy(), now);
    }
    history.reset();
    assert_eq!(
        history.crashed(&policy(), now),
        Ok(Duration::from_millis(500))
    );
}

#[test]
fn failed_starts_count_like_crashes_without_a_run() {
    let now = Instant::now();
    let mut history = CrashHistory::default();
    let delays: Vec<_> = (0..4)
        .map(|_| history.failed_start(&policy()).expect("retries"))
        .collect();
    assert_eq!(
        delays,
        [500, 1000, 2000, 2000].map(Duration::from_millis).to_vec()
    );
    history.started(now);
    let later = now + Duration::from_secs(61);
    assert_eq!(
        history.crashed(&policy(), later),
        Ok(Duration::from_millis(500)),
        "a stable run after failed starts starts the count again"
    );
    assert_eq!(
        history.failed_start(&policy()),
        Ok(Duration::from_millis(1000))
    );
}

#[test]
fn the_default_policy_restarts_five_times() {
    let policy = RestartPolicy::default();
    assert_eq!(policy.max_crashes, 5);
    assert_eq!(policy.base, Duration::from_millis(500));
}
