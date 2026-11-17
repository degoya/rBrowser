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
