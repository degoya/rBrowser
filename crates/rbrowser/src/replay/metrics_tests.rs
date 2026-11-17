use std::time::Duration;

use super::*;

fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}

#[test]
fn nearest_rank_percentiles() {
    let sorted: Vec<Duration> = (1..=100).map(ms).collect();
    assert_eq!(percentile(&sorted, 50.0), ms(50));
    assert_eq!(percentile(&sorted, 95.0), ms(95));
    assert_eq!(percentile(&sorted, 100.0), ms(100));
    assert_eq!(percentile(&sorted, 0.0), ms(1));
    assert_eq!(percentile(&[], 95.0), Duration::ZERO);
}

#[test]
fn the_summary_is_in_milliseconds() {
    let summary = summarize(&[ms(10), ms(30), ms(20)]);
    assert_eq!(summary.samples, 3);
    assert_eq!(summary.p50_ms, 20.0);
    assert_eq!(summary.p95_ms, 30.0);
    assert_eq!(summary.max_ms, 30.0);
}

#[test]
fn every_hundredth_replay_is_logged_and_the_window_slides() {
    let stats = LatencyStats::default();
    let logged: Vec<LatencySummary> = (0..(WINDOW as u64 + 100))
        .filter_map(|n| stats.record(ms(n)))
        .collect();
    assert_eq!(logged.len(), (WINDOW + 100) / LOG_EVERY as usize);
    let summary = stats.summary();
    assert_eq!(summary.samples, WINDOW);
    // The oldest 100 have left the window.
    assert_eq!(summary.max_ms, (WINDOW as f64 + 99.0));
    assert_eq!(percentile(&[ms(100)], 50.0), ms(100));
}
