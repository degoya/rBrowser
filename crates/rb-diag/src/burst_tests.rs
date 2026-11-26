use std::time::{Duration, Instant};

use super::*;

fn watch() -> BurstWatch {
    BurstWatch::new(Duration::from_secs(1), 10, Duration::from_secs(60))
}

#[test]
fn events_at_their_usual_pace_say_nothing() {
    let mut watch = watch();
    let start = Instant::now();
    for index in 0..100 {
        // Two per second: a grid built on a click, a pan, a new mode.
        assert_eq!(watch.note(start + Duration::from_millis(index * 500)), None);
    }
}

#[test]
fn more_than_the_limit_within_the_window_is_told_once() {
    let mut watch = watch();
    let start = Instant::now();
    let told: Vec<Option<usize>> = (0..30)
        .map(|frame| watch.note(start + Duration::from_millis(frame * 16)))
        .collect();
    assert!(told[..10].iter().all(Option::is_none));
    assert_eq!(told[10], Some(11));
    assert!(
        told[11..].iter().all(Option::is_none),
        "told again within the quiet time"
    );
}

#[test]
fn a_burst_that_goes_on_is_told_again_after_the_quiet_time() {
    let mut watch = watch();
    let start = Instant::now();
    let mut told = Vec::new();
    // 30 a second for two and a half minutes.
    for frame in 0..4_500_u64 {
        let now = start + Duration::from_millis(frame * 33);
        if let Some(count) = watch.note(now) {
            told.push((now - start, count));
        }
    }
    assert_eq!(told.len(), 3, "{told:?}");
    assert!(told.iter().all(|(_, count)| *count > 10));
    assert!(told[1].0 - told[0].0 >= Duration::from_secs(60));
}

#[test]
fn the_window_forgets_old_events() {
    let mut watch = watch();
    let start = Instant::now();
    for index in 0..10 {
        assert_eq!(watch.note(start + Duration::from_millis(index)), None);
    }
    // Ten more, two seconds later: the first ten are out of the window.
    for index in 0..10 {
        assert_eq!(
            watch.note(start + Duration::from_secs(2) + Duration::from_millis(index)),
            None
        );
    }
}
