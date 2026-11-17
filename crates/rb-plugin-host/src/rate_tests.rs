use std::time::{Duration, Instant};

use super::RateLimiter;

#[test]
fn requests_beyond_the_rate_wait_for_their_slot() {
    let limiter = RateLimiter::new(4);
    let start = Instant::now();
    assert_eq!(limiter.reserve_at(start), Duration::ZERO);
    assert_eq!(limiter.reserve_at(start), Duration::from_millis(250));
    assert_eq!(limiter.reserve_at(start), Duration::from_millis(500));
}

#[test]
fn an_idle_plugin_starts_again_without_waiting() {
    let limiter = RateLimiter::new(2);
    let start = Instant::now();
    limiter.reserve_at(start);
    assert_eq!(
        limiter.reserve_at(start + Duration::from_secs(5)),
        Duration::ZERO
    );
}

#[test]
fn a_rate_of_zero_is_one() {
    let limiter = RateLimiter::new(0);
    let start = Instant::now();
    limiter.reserve_at(start);
    assert_eq!(limiter.reserve_at(start), Duration::from_secs(1));
}
