use serde::{Deserialize, Serialize};

use super::*;

/// 2026-09-21 14:13:20 UTC and `nanos` more.
fn at(nanos: u32) -> OffsetDateTime {
    OffsetDateTime::from_unix_timestamp_nanos(1_790_000_000_000_000_000 + i128::from(nanos))
        .expect("time")
}

#[derive(Debug, Deserialize, Serialize)]
struct Stamped {
    #[serde(with = "crate::utc")]
    at: OffsetDateTime,
}

fn written(time: OffsetDateTime) -> String {
    serde_json::to_string(&Stamped { at: time }).expect("json")
}

fn read(json: &str) -> Result<OffsetDateTime, serde_json::Error> {
    serde_json::from_str::<Stamped>(json).map(|stamped| stamped.at)
}

/// RB-090-77: the forms chrono's `DateTime<Utc>` wrote - files, words and errors read as before.
#[test]
fn files_get_rfc_3339_with_z_and_the_fraction_in_threes() {
    assert_eq!(written(at(0)), r#"{"at":"2026-09-21T14:13:20Z"}"#);
    assert_eq!(
        written(at(500_000_000)),
        r#"{"at":"2026-09-21T14:13:20.500Z"}"#
    );
    assert_eq!(
        written(at(120_000)),
        r#"{"at":"2026-09-21T14:13:20.000120Z"}"#
    );
    assert_eq!(written(at(7)), r#"{"at":"2026-09-21T14:13:20.000000007Z"}"#);
}

#[test]
fn a_person_reads_plus_zero_and_an_error_reads_utc() {
    assert_eq!(rfc3339(at(0)), "2026-09-21T14:13:20+00:00");
    assert_eq!(rfc3339(at(250_000_000)), "2026-09-21T14:13:20.250+00:00");
    assert_eq!(display(&at(0)), "2026-09-21 14:13:20 UTC");
}

#[test]
fn any_offset_reads_as_the_same_instant_held_in_utc() {
    for text in [
        "2026-09-21T14:13:20Z",
        "2026-09-21T14:13:20z",
        "2026-09-21T16:13:20+02:00",
        "2026-09-21T09:13:20-05:00",
        "2026-09-21 14:13:20+00:00",
    ] {
        let time = parse(text).expect(text);
        assert_eq!(time, at(0), "{text}");
        assert_eq!(time.offset(), UtcOffset::UTC, "{text}");
        assert_eq!(rfc3339(time), "2026-09-21T14:13:20+00:00", "{text}");
    }
    assert_eq!(
        read(r#"{"at":"2026-09-21T15:13:20.5+01:00"}"#).expect("read"),
        at(500_000_000)
    );
}

#[test]
fn what_is_no_rfc_3339_time_is_refused() {
    for text in [
        "",
        "2026-09-21",
        "2026-09-21T14:13Z",
        "2026-09-21T14:13:20",
        "2026-13-21T14:13:20Z",
        "1790000000",
    ] {
        assert!(parse(text).is_err(), "{text}");
    }
    assert!(read(r#"{"at":1790000000}"#).is_err());
}
