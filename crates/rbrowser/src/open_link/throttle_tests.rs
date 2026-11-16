use std::time::{Duration, Instant};

use super::{MAX_OPENS, Throttle, WINDOW};

#[test]
fn a_burst_of_links_is_cut_and_the_window_lets_them_through_again() {
    let mut throttle = Throttle::default();
    let start = Instant::now();
    for step in 0..MAX_OPENS {
        assert!(throttle.allow(start + Duration::from_millis(step as u64)));
    }
    assert!(
        !throttle.allow(start + Duration::from_millis(10)),
        "one too many"
    );
    assert!(
        !throttle.allow(start + WINDOW - Duration::from_millis(1)),
        "still within the window"
    );
    assert!(
        throttle.allow(start + WINDOW),
        "the first one left the window"
    );
}

#[test]
fn a_link_that_did_not_open_gives_its_slot_back() {
    let mut throttle = Throttle::default();
    let start = Instant::now();
    for step in 0..MAX_OPENS {
        assert!(throttle.allow(start + Duration::from_millis(step as u64)));
    }
    throttle.release(start);
    assert!(throttle.allow(start + Duration::from_millis(50)));
    assert!(!throttle.allow(start + Duration::from_millis(60)));
}
