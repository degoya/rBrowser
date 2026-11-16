use std::time::{Duration, Instant};

use super::*;
use crate::watch::{Change, ChangeKind};

fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}

#[test]
fn fifty_changes_within_100_ms_make_exactly_one_batch() {
    let start = Instant::now();
    let mut debounce = Debouncer::new(ms(200));
    for i in 0..50u64 {
        let now = start + ms(i * 2);
        debounce.push(Change::new(format!("src/f{i}.js"), ChangeKind::Modify), now);
        assert_eq!(debounce.take(now), None, "nothing before the quiet time");
    }
    let last = start + ms(98);
    assert_eq!(debounce.take(last + ms(199)), None);
    let Some(Batch::Changes(changes)) = debounce.take(last + ms(200)) else {
        panic!("one batch after the quiet time");
    };
    assert_eq!(changes.len(), 50);
    assert_eq!(debounce.take(last + ms(5000)), None, "and only one");
    assert_eq!(debounce.deadline(), None);
}

#[test]
fn the_same_path_is_reported_once_with_its_last_kind() {
    let now = Instant::now();
    let mut debounce = Debouncer::new(ms(0));
    debounce.push(Change::new("a.css", ChangeKind::Create), now);
    debounce.push(Change::new("a.css", ChangeKind::Modify), now);
    debounce.push(Change::new("b.css", ChangeKind::Modify), now);
    assert_eq!(
        debounce.take(now),
        Some(Batch::Changes(vec![
            Change::new("a.css", ChangeKind::Modify),
            Change::new("b.css", ChangeKind::Modify),
        ]))
    );
}

#[test]
fn the_deadline_moves_with_every_change() {
    let start = Instant::now();
    let mut debounce = Debouncer::new(ms(200));
    assert_eq!(debounce.deadline(), None);
    debounce.push(Change::new("a.css", ChangeKind::Modify), start);
    assert_eq!(debounce.deadline(), Some(start + ms(200)));
    debounce.push(Change::new("a.css", ChangeKind::Modify), start + ms(150));
    assert_eq!(debounce.deadline(), Some(start + ms(350)));
}

#[test]
fn a_steady_stream_still_gets_out_after_the_longest_wait() {
    let start = Instant::now();
    let mut debounce = Debouncer::new(ms(200));
    let mut out = None;
    let mut t = 0;
    while out.is_none() && t < 10_000 {
        let now = start + ms(t);
        debounce.push(Change::new("log.txt", ChangeKind::Modify), now);
        out = debounce.take(now).map(|_| t);
        t += 50;
    }
    assert_eq!(out, Some(1000), "max(5 x 200 ms, 1 s)");
}

#[test]
fn a_rescan_replaces_the_changes() {
    let now = Instant::now();
    let mut debounce = Debouncer::new(ms(10));
    debounce.push(Change::new("a.css", ChangeKind::Modify), now);
    debounce.push_rescan(now);
    debounce.push(Change::new("b.css", ChangeKind::Modify), now);
    assert_eq!(debounce.take(now + ms(10)), Some(Batch::Rescan));
}

#[test]
fn too_many_changes_become_a_rescan() {
    let now = Instant::now();
    let mut debounce = Debouncer::new(ms(0));
    for i in 0..=MAX_BATCH {
        debounce.push(Change::new(format!("f{i}"), ChangeKind::Create), now);
    }
    assert_eq!(debounce.take(now), Some(Batch::Rescan));
}

#[test]
fn the_longest_wait_has_a_floor_of_one_second() {
    assert_eq!(longest_wait(ms(0)), Duration::from_secs(1));
    assert_eq!(longest_wait(ms(200)), Duration::from_secs(1));
    assert_eq!(longest_wait(ms(500)), Duration::from_millis(2500));
}
