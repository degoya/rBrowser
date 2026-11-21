//! The UTC instants of the plugin index as text (RB-090-77): RFC 3339 with `Z` in the index
//! (serde `with = "crate::utc"`), `+00:00` where a person reads one ([`rfc3339`]),
//! `2026-09-21 14:13:20 UTC` in an error ([`display`]) - the forms chrono's `DateTime<Utc>` wrote,
//! so the index and the messages read as before. A time read with any offset is held in UTC. The
//! same as `rb_update::utc`; neither crate depends on the other.

use serde::{Deserialize as _, Deserializer, Serializer};
use time::format_description::StaticFormatDescription;
use time::format_description::well_known::Rfc3339;
use time::macros::format_description;
use time::{OffsetDateTime, UtcOffset};

const DATE: StaticFormatDescription = format_description!("[year]-[month]-[day]");
const CLOCK: StaticFormatDescription = format_description!("[hour]:[minute]:[second]");

/// `text` as RFC 3339 with any offset, held in UTC.
///
/// # Errors
///
/// No RFC 3339 date and time, or one outside the years UTC can hold.
pub fn parse(text: &str) -> Result<OffsetDateTime, String> {
    OffsetDateTime::parse(text, &Rfc3339)
        .map_err(|error| format!("`{text}` is no RFC 3339 time: {error}"))?
        .checked_to_offset(UtcOffset::UTC)
        .ok_or_else(|| format!("`{text}` is out of range in UTC"))
}

/// RFC 3339 in UTC with `+00:00`: `2026-09-21T14:13:20+00:00`.
#[must_use]
pub fn rfc3339(time: OffsetDateTime) -> String {
    written(time, "T", "+00:00")
}

/// For an error or a log line: `2026-09-21 14:13:20 UTC`.
#[must_use]
pub fn display(time: &OffsetDateTime) -> String {
    written(*time, " ", " UTC")
}

/// serde `with`: RFC 3339 in UTC with `Z`.
///
/// # Errors
///
/// None of its own; the serializer's.
pub fn serialize<S: Serializer>(time: &OffsetDateTime, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(&written(*time, "T", "Z"))
}

/// serde `with`: [`parse`] of a string.
///
/// # Errors
///
/// No string, or no RFC 3339 time.
pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<OffsetDateTime, D::Error> {
    let text = String::deserialize(deserializer)?;
    parse(&text).map_err(<D::Error as serde::de::Error>::custom)
}

/// `time` in UTC: the date, `separator`, the clock, the fraction in three, six or nine digits
/// when there is one, then `suffix`.
fn written(time: OffsetDateTime, separator: &str, suffix: &str) -> String {
    let Some(utc) = time.checked_to_offset(UtcOffset::UTC) else {
        return time.to_string();
    };
    let (Ok(date), Ok(clock)) = (utc.format(DATE), utc.format(CLOCK)) else {
        return time.to_string();
    };
    let fraction = match utc.nanosecond() {
        0 => String::new(),
        nanos if nanos % 1_000_000 == 0 => format!(".{:03}", nanos / 1_000_000),
        nanos if nanos % 1_000 == 0 => format!(".{:06}", nanos / 1_000),
        nanos => format!(".{nanos:09}"),
    };
    format!("{date}{separator}{clock}{fraction}{suffix}")
}

#[cfg(test)]
#[path = "utc_tests.rs"]
mod tests;
