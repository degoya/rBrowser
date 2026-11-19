use super::*;

#[test]
fn a_pause_is_the_time_since_the_step_before_at_most_ten_seconds() {
    assert_eq!(MAX_PAUSE_MS, 10_000);
    assert_eq!(pause_before(None, 1_500), 0, "the first step waits nothing");
    assert_eq!(pause_before(Some(1_000), 1_400), 400);
    assert_eq!(pause_before(Some(0), 60_000), MAX_PAUSE_MS);
    // Steps moved in the panel can come before the one they follow in time.
    assert_eq!(pause_before(Some(2_000), 400), 0);
}

#[test]
fn the_pace_reads_as_the_setting_writes_it() {
    let read = |text: &str| serde_json::from_str::<Pace>(text).expect("a pace");
    assert_eq!(read("\"fast\""), Pace::Fast);
    assert_eq!(read("\"recorded\""), Pace::Recorded);
    assert_eq!(Pace::default(), Pace::Fast);
    assert!(serde_json::from_str::<Pace>("\"slow\"").is_err());
}

#[test]
fn the_pauses_keep_the_recorded_gaps_and_give_a_hover_its_dwell() {
    assert_eq!(HOVER_DWELL_MS, 500);
    // A navigation, a hover reported at the click's pointer-down 10 ms before it (the agent's
    // menu trigger, RB-090-76), the click, a fill 2 s later.
    let pauses = pauses(
        &[(0, false), (4_000, true), (4_010, false), (6_010, false)],
        0,
    );
    // The click waits the dwell after the hover; the hover gives it from its own pause, so the
    // run takes as long as the recording did.
    assert_eq!(pauses, vec![0, 3_510, 500, 2_000]);
    assert_eq!(pauses.iter().sum::<u64>(), 6_010);
}

#[test]
fn a_hover_without_a_pause_before_it_gives_no_dwell() {
    assert_eq!(pauses(&[(0, true), (5, false)], 0), vec![0, 5]);
    assert_eq!(
        pauses(&[(0, false), (200, true), (210, false)], 0),
        vec![0, 0, 210]
    );
    // A hover already apart from the step after it keeps its pauses.
    assert_eq!(
        pauses(&[(0, false), (1_000, true), (3_000, false)], 0),
        vec![0, 1_000, 2_000]
    );
    assert_eq!(pauses(&[], 0), Vec::<u64>::new());
}

#[test]
fn the_dwell_never_breaks_the_ten_second_cap() {
    // A pause cut to the cap still gives the hover's dwell from what is left.
    let pauses = pauses(&[(0, false), (60_000, true), (60_001, false)], 0);
    assert_eq!(pauses, vec![0, MAX_PAUSE_MS - 499, 500]);
}

#[test]
fn every_step_but_the_first_waits_at_least_the_least_pause() {
    // RB-090-84, `automation.recorder.minPauseMs`.
    assert_eq!(
        pauses(
            &[(0, false), (100, false), (29_000, false), (28_000, false)],
            2_000
        ),
        vec![0, 2_000, MAX_PAUSE_MS, 2_000]
    );
    // The dwell takes only what lies above the least pause.
    assert_eq!(
        pauses(&[(0, false), (800, true), (810, false)], 400),
        vec![0, 700, 500]
    );
    assert_eq!(
        pauses(&[(0, false), (100, true), (110, false)], 600),
        vec![0, 600, 600]
    );
    // A least pause above the cap is the cap.
    assert_eq!(
        pauses(&[(0, false), (1, false)], 60_000),
        vec![0, MAX_PAUSE_MS]
    );
}
