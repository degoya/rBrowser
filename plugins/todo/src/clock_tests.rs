use super::{Format, stamp};

/// 2026-11-16 14:05:09 UTC.
const NOW: u64 = 1_794_837_909;

#[test]
fn todo_plus_writes_two_digit_years_and_minutes_by_default() {
    assert_eq!(stamp(NOW, 0, Format::TodoPlus), "26-11-16 14:05");
    assert_eq!(stamp(NOW, 0, Format::default()), "26-11-16 14:05");
    assert_eq!(stamp(NOW, 0, Format::FullYear), "2026-11-16 14:05");
    assert_eq!(stamp(NOW, 0, Format::Date), "26-11-16");
    assert_eq!(stamp(NOW, 0, Format::FullDate), "2026-11-16");
}

#[test]
fn the_callers_offset_makes_it_local_time() {
    assert_eq!(stamp(NOW, 60, Format::TodoPlus), "26-11-16 15:05");
    assert_eq!(stamp(NOW, -14 * 60, Format::TodoPlus), "26-11-16 00:05");
    assert_eq!(
        stamp(NOW - 3600, -14 * 60, Format::TodoPlus),
        "26-11-15 23:05"
    );
    assert_eq!(stamp(NOW, 10 * 60, Format::TodoPlus), "26-11-17 00:05");
    // Held to UTC+-14 h.
    assert_eq!(
        stamp(NOW, 100 * 60, Format::TodoPlus),
        stamp(NOW, 14 * 60, Format::TodoPlus)
    );
}

#[test]
fn the_epoch_and_a_leap_day_read_right() {
    assert_eq!(stamp(0, 0, Format::FullYear), "1970-01-01 00:00");
    // 2024-02-29 12:00 UTC.
    assert_eq!(stamp(1_709_208_000, 0, Format::FullDate), "2024-02-29");
    assert_eq!(
        stamp(1_709_208_000 + 86_400, 0, Format::FullDate),
        "2024-03-01"
    );
}

#[test]
fn the_settings_name_the_formats() {
    assert_eq!(Format::parse("todoPlus"), Some(Format::TodoPlus));
    assert_eq!(Format::parse("fullDate"), Some(Format::FullDate));
    assert_eq!(Format::parse("YY-MM-DD"), None);
}
