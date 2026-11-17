use super::*;

// 2026-07-20 10:00:00 UTC and 2026-11-10 12:03:07 UTC: summer and winter time.
const SUMMER: u64 = 1_784_541_600;
const WINTER: u64 = 1_794_312_187;

#[test]
fn local_names_use_the_offset_of_a_real_zone() {
    for secs in [SUMMER, WINTER] {
        let offset = offset_in(FileNameTime::Local, secs);
        assert!(offset.abs() <= 14 * 3600, "{offset}");
        assert_eq!(
            stamp_in(FileNameTime::Local, secs),
            Stamp::from_unix_with_offset(secs, offset)
        );
    }
}

#[test]
fn utc_names_ignore_the_local_zone() {
    for secs in [SUMMER, WINTER] {
        assert_eq!(offset_in(FileNameTime::Utc, secs), 0);
        assert_eq!(stamp_in(FileNameTime::Utc, secs), Stamp::from_unix(secs));
    }
    assert_eq!(
        stamp_in(FileNameTime::Utc, WINTER).date_time(),
        "2026-11-10_12-03-07"
    );
}

#[test]
fn now_is_the_stamp_of_the_clock_in_the_zone_in_effect() {
    let zone = zone();
    let before = now_secs();
    let stamp = stamp_now();
    let after = now_secs();
    assert!(stamp == stamp_in(zone, before) || stamp == stamp_in(zone, after));
}
