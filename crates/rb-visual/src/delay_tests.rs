use std::time::Duration;

use super::*;

#[test]
fn the_cycle_is_0_5_10() {
    assert_eq!(Delay::None.next(), Delay::Five);
    assert_eq!(Delay::Five.next(), Delay::Ten);
    assert_eq!(Delay::Ten.next(), Delay::None);
}

#[test]
fn seconds_and_durations() {
    assert_eq!(Delay::from_secs(0), Some(Delay::None));
    assert_eq!(Delay::from_secs(5), Some(Delay::Five));
    assert_eq!(Delay::from_secs(10), Some(Delay::Ten));
    assert_eq!(Delay::from_secs(7), None);
    assert_eq!(Delay::Five.duration(), Duration::from_secs(5));
    assert_eq!(Delay::Ten.secs(), 10);
}

#[test]
fn a_delay_in_ms_is_capped() {
    assert_eq!(clamp_delay_ms(5_000), Duration::from_millis(5_000));
    assert_eq!(
        clamp_delay_ms(u64::MAX),
        Duration::from_millis(MAX_DELAY_MS)
    );
}

#[test]
fn the_countdown_ticks_once_a_second_down_to_one() {
    assert_eq!(countdown(Duration::from_secs(5)), vec![5, 4, 3, 2, 1]);
    assert_eq!(countdown(Duration::from_millis(2_500)), vec![3, 2, 1]);
    assert!(countdown(Duration::ZERO).is_empty());
}
