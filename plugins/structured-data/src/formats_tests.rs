use super::*;

#[test]
fn dates_are_years_months_or_days() {
    for good in ["2026", "2026-11", "2026-11-20"] {
        assert!(is_date(good), "{good}");
    }
    for bad in [
        "26-11-20",
        "2026-13",
        "2026-11-32",
        "2026/11/20",
        "next friday",
        "",
    ] {
        assert!(!is_date(bad), "{bad}");
    }
}

#[test]
fn times_take_seconds_fractions_and_zones() {
    for good in [
        "09:30",
        "09:30:15",
        "09:30:15.250",
        "09:30Z",
        "09:30:00+01:00",
        "23:59-0500",
    ] {
        assert!(is_time(good), "{good}");
    }
    for bad in ["9:30", "09:61", "09:30:15.", "09:30+1", "noon"] {
        assert!(!is_time(bad), "{bad}");
    }
}

#[test]
fn a_date_time_is_a_full_date_with_an_optional_time() {
    for good in [
        "2026-11-20",
        "2026-11-20T09:30",
        "2026-11-20 09:30:00Z",
        "2026-11-20T09:30:00.5+01:00",
    ] {
        assert!(is_date_time(good), "{good}");
    }
    for bad in ["2026-11", "2026-11-20T", "2026-11-20Tlate", "tomorrow"] {
        assert!(!is_date_time(bad), "{bad}");
    }
}

#[test]
fn booleans_are_true_and_false_in_any_spelling_schema_org_gives() {
    for good in ["true", "False", "https://schema.org/True", "schema:False"] {
        assert!(is_boolean(good), "{good}");
    }
    for bad in ["yes", "1", "maybe"] {
        assert!(!is_boolean(bad), "{bad}");
    }
}
