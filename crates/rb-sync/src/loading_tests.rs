use std::time::{Duration, Instant};

use super::*;

const A: ViewId = ViewId(1);
const B: ViewId = ViewId(2);

#[test]
fn a_given_load_ends_with_the_end_of_a_load_after_it() {
    let now = Instant::now();
    let mut table = LoadTable::default();
    table.host_state(A, false, 4);
    table.mark(A, now);
    assert!(table.is_loading(A, now));
    assert_eq!(table.host_state(A, true, 4), Some(LoadChange::Started));
    assert_eq!(table.host_state(A, true, 4), None, "no change");
    // Started: no grace runs out while it loads.
    assert!(table.is_loading(A, now + LOAD_START_GRACE * 10));
    assert_eq!(table.host_state(A, false, 5), Some(LoadChange::Finished));
    assert!(!table.is_loading(A, now));
    assert_eq!(table.host_state(A, false, 5), None);
}

#[test]
fn an_end_is_seen_when_start_and_end_came_before_the_look() {
    // The host flipped to loading and back before the hub read the view: only its count of
    // ended loads says so.
    let now = Instant::now();
    let mut table = LoadTable::default();
    table.mark(A, now);
    assert_eq!(table.host_state(A, false, 1), Some(LoadChange::Finished));
    assert!(!table.is_loading(A, now));
}

#[test]
fn a_load_that_never_starts_ends_after_the_grace() {
    let now = Instant::now();
    let mut table = LoadTable::default();
    table.mark(A, now);
    // A title change: the view is read, nothing ended.
    assert_eq!(table.host_state(A, false, 0), None);
    assert!(table.is_loading(A, now + Duration::from_millis(10)));
    assert!(!table.is_loading(A, now + LOAD_START_GRACE));
    table.expire(now + LOAD_START_GRACE);
    assert!(!table.is_loading(A, now), "expired, not only out of time");
}

#[test]
fn a_load_given_while_the_view_loads_counts_as_started() {
    let now = Instant::now();
    let mut table = LoadTable::default();
    table.host_state(A, true, 0);
    table.mark(A, now);
    assert!(table.is_loading(A, now + LOAD_START_GRACE * 2));
    table.host_state(A, false, 1);
    assert!(!table.is_loading(A, now));
}

#[test]
fn another_view_s_load_does_not_end_it() {
    let now = Instant::now();
    let mut table = LoadTable::default();
    table.mark(B, now);
    table.host_state(A, true, 0);
    table.host_state(A, false, 1);
    assert_eq!(table.host_state(B, false, 0), None);
    assert!(table.is_loading(B, now));
    assert!(table.clear(B));
    assert!(!table.clear(B));
}

#[test]
fn views_that_are_gone_are_forgotten() {
    let now = Instant::now();
    let mut table = LoadTable::default();
    table.mark(A, now);
    table.host_state(B, true, 3);
    table.retain(&[]);
    assert!(!table.is_loading(A, now));
    assert_eq!(
        table.host_state(B, true, 3),
        Some(LoadChange::Finished),
        "B's count was forgotten"
    );
}
