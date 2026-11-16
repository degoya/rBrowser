use super::*;

fn pacer(fps: u32) -> Pacer {
    Pacer::new(fps, 300.0)
}

#[test]
fn the_first_frame_starts_the_clock_and_writes_nothing_yet() {
    let mut pacer = pacer(30);
    assert_eq!(pacer.advance(100.0), 0);
    assert_eq!(pacer.written(), 0);
}

/// A frame is held until the next one comes: each slot of 1/fps before the next frame's time is
/// one copy of the held frame.
#[test]
fn a_frame_is_repeated_for_every_slot_until_the_next() {
    let mut pacer = pacer(10);
    assert_eq!(pacer.advance(100.0), 0);
    // 0.25 s later: slots 0, 1 and 2 (at 0.0, 0.1, 0.2 s) show the first frame.
    assert_eq!(pacer.advance(100.25), 3);
    // 0.05 s later: no slot passed (the next is at 0.3 s).
    assert_eq!(pacer.advance(100.30), 0);
    assert_eq!(pacer.advance(100.31), 1);
    assert_eq!(pacer.written(), 4);
}

/// A static page sends one frame and nothing more: the stop time fills the file to its length.
#[test]
fn the_stop_time_fills_the_rest_with_the_last_frame() {
    let mut pacer = pacer(30);
    pacer.advance(50.0);
    assert_eq!(pacer.finish(60.0), 300, "10 s at 30 fps");
    assert_eq!(pacer.written(), 300);
    assert!((pacer.duration_secs() - 10.0).abs() < 1e-9);
}

#[test]
fn the_last_frame_is_written_at_least_once() {
    let mut pacer = pacer(30);
    pacer.advance(10.0);
    assert_eq!(pacer.advance(10.0), 0);
    assert_eq!(pacer.finish(10.0), 1);
}

#[test]
fn without_any_frame_nothing_is_written() {
    let mut pacer = pacer(30);
    assert_eq!(pacer.finish(10.0), 0);
}

#[test]
fn a_frame_from_the_past_writes_nothing() {
    let mut pacer = pacer(10);
    pacer.advance(100.0);
    assert_eq!(pacer.advance(101.0), 10);
    assert_eq!(pacer.advance(99.0), 0);
    assert_eq!(pacer.written(), 10);
}

/// The file never runs longer than the maximum, whatever the timestamps say.
#[test]
fn the_slots_stop_at_the_maximum_duration() {
    let mut pacer = Pacer::new(10, 2.0);
    pacer.advance(0.0);
    assert_eq!(pacer.advance(3600.0), 20);
    assert_eq!(pacer.finish(7200.0), 0);
    assert_eq!(pacer.written(), 20);
}

#[test]
fn the_frame_rate_is_held_to_its_bounds() {
    assert_eq!(Pacer::new(0, 10.0).fps(), 1);
    assert_eq!(Pacer::new(500, 10.0).fps(), MAX_FPS);
}

#[test]
fn a_non_finite_timestamp_writes_nothing() {
    let mut pacer = pacer(10);
    assert_eq!(pacer.advance(f64::NAN), 0);
    pacer.advance(1.0);
    assert_eq!(pacer.advance(f64::INFINITY), 0);
}
