use std::time::{Duration, SystemTime};

use super::Activity;

#[test]
fn nothing_reached_a_new_server() {
    let activity = Activity::new();
    assert_eq!(activity.since_last(SystemTime::now()), (None, 0));
}

#[test]
fn the_last_request_and_the_count_are_kept_until_a_restart() {
    let activity = Activity::new();
    let start = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000);
    activity.record(start);
    activity.record(start + Duration::from_secs(5));
    let (ago, calls) = activity.since_last(start + Duration::from_secs(12));
    assert_eq!(ago, Some(Duration::from_secs(7)));
    assert_eq!(calls, 2);
    activity.reset();
    assert_eq!(activity.since_last(start), (None, 0));
}

#[test]
fn a_clock_that_went_back_says_just_now() {
    let activity = Activity::new();
    let at = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000);
    activity.record(at);
    // An older record never moves the last request back.
    activity.record(at - Duration::from_secs(60));
    let (ago, calls) = activity.since_last(at - Duration::from_secs(30));
    assert_eq!(ago, Some(Duration::ZERO));
    assert_eq!(calls, 2);
}
