use super::*;
use crate::rng::Rng;

#[test]
fn dates_round_trip_through_days() {
    for text in ["1970-01-01", "2000-02-29", "2026-11-12", "1899-12-31"] {
        let days = Scale::Date.parse(text).expect(text);
        assert_eq!(Scale::Date.format(days), text);
    }
    assert_eq!(Scale::Date.parse("1970-01-02"), Some(1));
    assert_eq!(Scale::Date.parse("2026-13-01"), None);
    assert_eq!(Scale::Date.parse("soon"), None);
}

#[test]
fn times_months_weeks_and_datetimes_round_trip() {
    assert_eq!(Scale::Time.parse("09:30"), Some(570));
    assert_eq!(Scale::Time.parse("09:30:15"), Some(570));
    assert_eq!(Scale::Time.format(570), "09:30");
    assert_eq!(Scale::Time.parse("24:00"), None);
    for (scale, text) in [
        (Scale::Month, "2026-05"),
        (Scale::Week, "2026-W20"),
        (Scale::Week, "2021-W01"),
        (Scale::Week, "2020-W53"),
        (Scale::DateTime, "2026-05-14T09:30"),
    ] {
        let units = scale.parse(text).expect(text);
        assert_eq!(scale.format(units), text);
    }
    let w1 = Scale::Week.parse("2026-W01").expect("week");
    assert_eq!(Scale::Week.format(w1 + 7), "2026-W02");
    assert_eq!(Scale::Week.parse("2026-W54"), None);
}

#[test]
fn a_valid_value_stays_within_min_and_max() {
    for seed in 0..40 {
        let mut rng = Rng::new(seed);
        let values = valid_candidates(Scale::Date, "2024-01-10", "2024-01-20", "", &mut rng);
        let first = Scale::Date.parse(&values[0]).expect("date");
        assert!(first >= Scale::Date.parse("2024-01-10").expect("min"));
        assert!(first <= Scale::Date.parse("2024-01-20").expect("max"));
        assert!(values.contains(&"2024-01-10".to_owned()));
    }
    let mut rng = Rng::new(3);
    let only_max = valid_candidates(Scale::Date, "", "2008-01-01", "", &mut rng);
    assert!(Scale::Date.parse(&only_max[0]) <= Scale::Date.parse("2008-01-01"));
}

#[test]
fn numbers_keep_min_max_and_step() {
    for seed in 0..60 {
        let mut rng = Rng::new(seed);
        let values = valid_candidates(Scale::Number, "18", "120", "", &mut rng);
        let n: f64 = values[0].parse().expect("number");
        assert!((18.0..=120.0).contains(&n) && n.fract() == 0.0, "{n}");
        let stepped = valid_candidates(Scale::Number, "1", "9", "2", &mut rng);
        let n: i64 = stepped[0].parse().expect("integer");
        assert!(n % 2 == 1 && (1..=9).contains(&n), "{n}");
        let tenths = valid_candidates(Scale::Number, "0", "1", "0.1", &mut rng);
        let n: f64 = tenths[0].parse().expect("decimal");
        assert!((0.0..=1.0).contains(&n));
        assert!(tenths[0].len() <= 3, "{}", tenths[0]);
    }
}

#[test]
fn a_number_without_bounds_is_small_and_positive() {
    let mut rng = Rng::new(1);
    let n: i64 = valid_candidates(Scale::Number, "", "", "", &mut rng)[0]
        .parse()
        .expect("integer");
    assert!((1..=100).contains(&n));
}

#[test]
fn bounds_are_broken_one_unit_out() {
    assert_eq!(
        out_of_bounds(Scale::Number, "18", "120", ""),
        Some("17".to_owned())
    );
    assert_eq!(
        out_of_bounds(Scale::Number, "", "120", ""),
        Some("121".to_owned())
    );
    assert_eq!(
        out_of_bounds(Scale::Number, "0", "", "0.5"),
        Some("-0.5".to_owned())
    );
    assert_eq!(
        out_of_bounds(Scale::Date, "2024-01-10", "", ""),
        Some("2024-01-09".to_owned())
    );
    assert_eq!(
        out_of_bounds(Scale::Time, "", "18:00", ""),
        Some("18:01".to_owned())
    );
    assert_eq!(out_of_bounds(Scale::Number, "", "", ""), None);
}

#[test]
fn a_step_is_broken_halfway() {
    assert_eq!(off_step("1", "", "2"), Some("2".to_owned()));
    assert_eq!(off_step("", "", "10"), Some("5".to_owned()));
    assert_eq!(off_step("", "", "any"), None);
    assert_eq!(off_step("", "", ""), None);
    assert_eq!(off_step("", "", "1"), Some("0.5".to_owned()));
}

#[test]
fn numbers_print_without_float_noise() {
    assert_eq!(number_text(0.30000000000000004, 1), "0.3");
    assert_eq!(number_text(42.0, 0), "42");
    assert_eq!(number_text(-0.5, 1), "-0.5");
    assert_eq!(decimals("0.25"), 2);
    assert_eq!(decimals("5"), 0);
}
