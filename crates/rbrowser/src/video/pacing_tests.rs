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

/// RB-090-68: a pause from 101 to 104 is left out - the frame held at the pause is written up
/// to the pause, the next one from there, and the file is 3 s shorter than the wall clock.
#[test]
fn a_pause_is_left_out_of_the_slots() {
    let mut pacer = pacer(10);
    let mut pauses = Pauses::default();
    assert!(pauses.pause(101.0));
    assert!(pauses.resume(104.0));
    pacer.set_pauses(pauses);
    assert_eq!(pacer.advance(100.0), 0);
    // The frame of 100.0 until the pause at 101.0: 10 slots.
    assert_eq!(pacer.advance(102.0), 10);
    // A frame during the pause writes nothing: the pause has no slots.
    assert_eq!(pacer.advance(103.0), 0);
    // After the resume the frame held since the pause runs on: 104.5 is 1.5 s recorded.
    assert_eq!(pacer.advance(104.5), 5);
    assert_eq!(pacer.finish(105.0), 5);
    assert_eq!(pacer.written(), 20);
    assert!((pacer.duration_secs() - 2.0).abs() < 1e-9);
}

/// A pause that is still open stops the clock at its start: frames and the stop during it add
/// nothing past the pause.
#[test]
fn an_open_pause_stops_the_clock_at_its_start() {
    let mut pacer = pacer(10);
    pacer.advance(100.0);
    let mut pauses = Pauses::default();
    assert!(pauses.pause(100.5));
    pacer.set_pauses(pauses);
    assert_eq!(pacer.advance(130.0), 5);
    // The frame painted during the pause is the last: written once, as every last frame is.
    assert_eq!(pacer.finish(160.0), 1);
    assert_eq!(pacer.written(), 6);
}

/// The pauses' log refuses what makes no sense: a second pause while paused, a resume without a
/// pause, a time that is no number; a resume before its pause counts as none.
#[test]
fn the_pauses_take_only_sensible_changes() {
    let mut pauses = Pauses::default();
    assert!(!pauses.is_paused());
    assert!(!pauses.resume(1.0));
    assert!(!pauses.pause(f64::NAN));
    assert!(pauses.pause(10.0));
    assert!(!pauses.pause(11.0), "already paused");
    assert!(pauses.is_paused());
    assert_eq!(pauses.paused_since(), Some(10.0));
    assert!(
        (pauses.before(15.0) - 5.0).abs() < 1e-9,
        "an open pause runs to the time asked"
    );
    assert!(pauses.ended_secs().abs() < 1e-9);
    assert!(pauses.resume(9.0));
    assert!(
        pauses.ended_secs().abs() < 1e-9,
        "a resume before its pause is a pause of 0 s"
    );
    assert!(pauses.pause(20.0));
    assert!(pauses.resume(22.5));
    assert!((pauses.ended_secs() - 2.5).abs() < 1e-9);
    assert!((pauses.before(21.0) - 1.0).abs() < 1e-9);
    assert!(pauses.before(5.0).abs() < 1e-9);
    assert_eq!(pauses.paused_since(), None);
    // A pause never starts before the last one ended: no time counts twice.
    assert!(pauses.pause(21.0));
    assert_eq!(pauses.paused_since(), Some(22.5));
}
