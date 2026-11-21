use std::time::Duration;

use tokio::time::Instant;

use super::*;

#[test]
fn slots_are_spaced_by_the_rate() {
    let start = Instant::now();
    let mut pace = Pace::new(4.0).expect("a rate");
    assert_eq!(pace.slot(start), start);
    assert_eq!(pace.slot(start), start + Duration::from_millis(250));
    assert_eq!(pace.slot(start), start + Duration::from_millis(500));
}

#[test]
fn a_quiet_time_saves_up_no_burst() {
    let start = Instant::now();
    let mut pace = Pace::new(10.0).expect("a rate");
    assert_eq!(pace.slot(start), start);
    let later = start + Duration::from_secs(5);
    assert_eq!(pace.slot(later), later);
    assert_eq!(pace.slot(later), later + Duration::from_millis(100));
}

#[test]
fn no_rate_is_no_pace() {
    assert!(Pace::new(0.0).is_none());
    assert!(Pace::new(-1.0).is_none());
    assert!(Pace::new(f64::INFINITY).is_none());
    assert!(Pace::new(f64::NAN).is_none());
}
