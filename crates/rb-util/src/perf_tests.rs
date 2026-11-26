use std::time::Duration;

use super::{Gauge, Registry, WINDOW, names, percentile};

fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}

#[test]
fn a_series_answers_count_percentiles_max_and_mean() {
    let registry = Registry::new();
    for n in 1..=100 {
        registry.record(names::IPC_QUERY, ms(n));
    }
    let snapshot = registry.snapshot();
    assert_eq!(snapshot.len(), 1);
    let ipc = &snapshot[0];
    assert_eq!(ipc.name, names::IPC_QUERY);
    assert_eq!(ipc.count, 100);
    assert_eq!(ipc.window, 100);
    assert_eq!(ipc.p50, ms(50));
    assert_eq!(ipc.p95, ms(95));
    assert_eq!(ipc.max, ms(100));
    assert_eq!(ipc.mean, Duration::from_micros(50_500));
}

#[test]
fn the_snapshot_is_sorted_by_name() {
    let registry = Registry::new();
    registry.record(names::SYNC_WAIT, ms(1));
    registry.record(names::IPC_QUERY, ms(1));
    registry.record(names::PLUGIN_CALL, ms(1));
    let order: Vec<&str> = registry.snapshot().iter().map(|s| s.name).collect();
    assert_eq!(order, vec!["ipc.query", "plugin.call", "sync.wait"]);
}

#[test]
fn the_percentiles_cover_the_last_window_while_the_count_goes_on() {
    let registry = Registry::new();
    for _ in 0..WINDOW {
        registry.record(names::SYNC_WAIT, ms(500));
    }
    for _ in 0..WINDOW {
        registry.record(names::SYNC_WAIT, ms(2));
    }
    let wait = &registry.snapshot()[0];
    assert_eq!(wait.count, 2 * WINDOW as u64);
    assert_eq!(wait.window, WINDOW);
    assert_eq!(wait.p95, ms(2));
    assert_eq!(wait.max, ms(2), "the slow ones left the window");
}

#[test]
fn switched_off_it_records_nothing_and_on_again_it_does() {
    let registry = Registry::new();
    assert!(registry.enabled(), "on by default");
    registry.set_enabled(false);
    registry.record(names::IPC_QUERY, ms(3));
    assert!(registry.snapshot().is_empty());
    registry.set_enabled(true);
    registry.record(names::IPC_QUERY, ms(3));
    assert_eq!(registry.snapshot()[0].count, 1);
}

#[test]
fn record_once_keeps_the_first_value() {
    let registry = Registry::new();
    assert!(registry.record_once(names::STARTUP_FIRST_VIEW, ms(900)));
    assert!(!registry.record_once(names::STARTUP_FIRST_VIEW, ms(5)));
    let first = &registry.snapshot()[0];
    assert_eq!(first.count, 1);
    assert_eq!(first.max, ms(900));
}

#[test]
fn reset_forgets_every_series_but_not_a_record_once() {
    let registry = Registry::new();
    registry.record(names::IPC_QUERY, ms(1));
    registry.record_once(names::STARTUP_FIRST_VIEW, ms(700));
    registry.reset();
    let left: Vec<&str> = registry.snapshot().iter().map(|s| s.name).collect();
    assert_eq!(left, vec![names::STARTUP_FIRST_VIEW]);
    assert!(!registry.record_once(names::STARTUP_FIRST_VIEW, ms(1)));
}

#[test]
fn the_nearest_rank_percentile_of_none_is_zero() {
    assert_eq!(percentile(&[], 95.0), Duration::ZERO);
    assert_eq!(percentile(&[ms(7)], 50.0), ms(7));
    assert_eq!(percentile(&[ms(1), ms(2), ms(3), ms(4)], 50.0), ms(2));
}

#[test]
fn the_engine_names_are_one_per_engine() {
    let all = [
        names::SYNC_REPLAY_CHROMIUM,
        names::SYNC_REPLAY_FIREFOX,
        names::SYNC_REPLAY_WEBKIT,
        names::SYNC_REPLAY_ANDROID,
        names::SYNC_REPLAY_SAFARI,
        names::SYNC_REPLAY_IOS,
    ];
    let mut unique = all.to_vec();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(unique.len(), all.len());
    assert!(all.iter().all(|name| name.starts_with("sync.replay.")));
}

#[test]
fn the_global_registry_and_the_start_mark_are_one_each() {
    let first = super::mark_start();
    assert_eq!(super::mark_start(), first);
    super::record(names::PLUGIN_WAIT, ms(4));
    assert!(
        super::global()
            .snapshot()
            .iter()
            .any(|series| series.name == names::PLUGIN_WAIT && series.count >= 1)
    );
}

/// RB-099-19: a gauge goes up and down, never below zero, and keeps its highest level.
#[test]
fn a_gauge_tracks_its_level_and_its_peak() {
    let gauge = Gauge::new();
    assert_eq!(gauge.add(100), 100);
    assert_eq!(gauge.add(50), 150);
    gauge.sub(120);
    assert_eq!((gauge.now(), gauge.peak()), (30, 150));
    gauge.sub(1_000);
    assert_eq!((gauge.now(), gauge.peak()), (0, 150));
}

#[test]
fn a_reset_gauge_is_at_zero_and_keeps_its_peak() {
    let gauge = Gauge::new();
    gauge.add(40);
    gauge.reset();
    assert_eq!((gauge.now(), gauge.peak()), (0, 40));
}
