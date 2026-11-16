use std::time::{Duration, Instant};

use super::*;

fn s(n: u64) -> Duration {
    Duration::from_secs(n)
}

#[test]
fn it_reloads_every_ten_seconds() {
    let start = Instant::now();
    let mut interval = IntervalReload::new(s(10), s(5), start);
    assert_eq!(interval.tick(start), Tick::Wait(s(10)));
    assert_eq!(interval.tick(start + s(9)), Tick::Wait(s(1)));
    assert_eq!(interval.tick(start + s(10)), Tick::Due);
    interval.reloaded(start + s(10));
    assert_eq!(interval.tick(start + s(15)), Tick::Wait(s(5)));
    assert_eq!(interval.tick(start + s(20)), Tick::Due);
}

#[test]
fn input_pauses_it() {
    let start = Instant::now();
    let mut interval = IntervalReload::new(s(10), s(5), start);
    interval.note_input(start + s(8));
    assert_eq!(interval.tick(start + s(10)), Tick::Wait(s(3)));
    interval.note_input(start + s(12));
    assert_eq!(interval.tick(start + s(13)), Tick::Wait(s(4)));
    assert_eq!(interval.tick(start + s(17)), Tick::Due);
}

#[test]
fn early_input_does_not_delay_it() {
    let start = Instant::now();
    let mut interval = IntervalReload::new(s(10), s(5), start);
    interval.note_input(start + s(1));
    assert_eq!(interval.tick(start + s(10)), Tick::Due);
    assert_eq!(interval.period(), s(10));
}
