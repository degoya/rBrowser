use std::time::{Duration, Instant};

use serde_json::json;

use super::*;

const LIMITS: ResourceLimits = ResourceLimits {
    ram_warn_mb: 500,
    cpu_warn_percent: 50,
};

fn metrics(timestamp: f64, task: f64, heap_mb: f64) -> PageMetrics {
    PageMetrics {
        timestamp_s: timestamp,
        task_duration_s: task,
        js_heap_bytes: heap_mb * 1_048_576.0,
    }
}

#[test]
fn the_cdp_answer_gives_time_tasks_and_heap() {
    let answer = json!({ "metrics": [
        { "name": "Timestamp", "value": 1000.5 },
        { "name": "TaskDuration", "value": 2.25 },
        { "name": "JSHeapTotalSize", "value": 10_485_760.0 },
        { "name": "Nodes", "value": 120 }
    ]});
    assert_eq!(
        parse_metrics(&answer),
        Some(PageMetrics {
            timestamp_s: 1000.5,
            task_duration_s: 2.25,
            js_heap_bytes: 10_485_760.0
        })
    );
    assert_eq!(parse_metrics(&json!({ "metrics": [] })), None);
    assert_eq!(parse_metrics(&json!({})), None);
}

#[test]
fn the_first_sample_has_memory_and_no_cpu_yet() {
    let mut view = ViewResources::default();
    let (reading, changes) = view.sample(metrics(10.0, 1.0, 48.0), Instant::now(), LIMITS);
    assert_eq!(reading.cpu_percent, None);
    assert_eq!(reading.memory_mb, Some(48.0));
    assert!(!reading.ram_warn && !reading.cpu_warn);
    assert!(changes.is_empty());
}

#[test]
fn cpu_is_the_busy_share_of_the_interval() {
    let mut view = ViewResources::default();
    let start = Instant::now();
    view.sample(metrics(10.0, 1.0, 48.0), start, LIMITS);
    let (reading, _) = view.sample(
        metrics(12.0, 1.5, 48.0),
        start + Duration::from_secs(2),
        LIMITS,
    );
    assert_eq!(reading.cpu_percent, Some(25.0));
}

#[test]
fn a_reset_counter_gives_no_cpu_for_that_interval() {
    let mut view = ViewResources::default();
    let start = Instant::now();
    view.sample(metrics(10.0, 5.0, 48.0), start, LIMITS);
    let (reading, _) = view.sample(
        metrics(12.0, 0.1, 20.0),
        start + Duration::from_secs(2),
        LIMITS,
    );
    assert_eq!(reading.cpu_percent, None);
    assert_eq!(reading.memory_mb, Some(20.0));
}

#[test]
fn memory_above_the_limit_warns_once_and_calms_once() {
    let mut view = ViewResources::default();
    let start = Instant::now();
    let (reading, changes) = view.sample(metrics(1.0, 0.0, 600.0), start, LIMITS);
    assert!(reading.ram_warn);
    assert_eq!(changes, vec![Change::RamWarn(600.0)]);
    let (_, again) = view.sample(metrics(3.0, 0.0, 601.0), start, LIMITS);
    assert!(again.is_empty());
    let (calm, back) = view.sample(metrics(5.0, 0.0, 100.0), start, LIMITS);
    assert!(!calm.ram_warn);
    assert_eq!(back, vec![Change::RamCalm]);
}

#[test]
fn cpu_warns_only_after_five_seconds_above_the_limit() {
    let mut view = ViewResources::default();
    let start = Instant::now();
    let at = |seconds: u64| start + Duration::from_secs(seconds);
    view.sample(metrics(0.0, 0.0, 10.0), at(0), LIMITS);
    let (busy, changes) = view.sample(metrics(2.0, 1.6, 10.0), at(2), LIMITS);
    assert_eq!(busy.cpu_percent, Some(80.0));
    assert!(!busy.cpu_warn && changes.is_empty());
    let (still, _) = view.sample(metrics(4.0, 3.2, 10.0), at(4), LIMITS);
    assert!(!still.cpu_warn);
    let (warned, changes) = view.sample(metrics(7.0, 5.6, 10.0), at(7), LIMITS);
    assert!(warned.cpu_warn);
    assert_eq!(changes, vec![Change::CpuWarn(80.0)]);
    let (calm, changes) = view.sample(metrics(9.0, 5.7, 10.0), at(9), LIMITS);
    assert!(!calm.cpu_warn);
    assert_eq!(changes, vec![Change::CpuCalm]);
}

#[test]
fn a_dip_below_the_limit_starts_the_wait_again() {
    let mut view = ViewResources::default();
    let start = Instant::now();
    let at = |seconds: u64| start + Duration::from_secs(seconds);
    view.sample(metrics(0.0, 0.0, 10.0), at(0), LIMITS);
    view.sample(metrics(2.0, 1.6, 10.0), at(2), LIMITS);
    view.sample(metrics(4.0, 1.7, 10.0), at(4), LIMITS);
    let (busy, _) = view.sample(metrics(6.0, 3.3, 10.0), at(6), LIMITS);
    assert!(!busy.cpu_warn);
    let (busy, _) = view.sample(metrics(9.0, 5.7, 10.0), at(9), LIMITS);
    assert!(!busy.cpu_warn, "only three seconds since the dip");
}
