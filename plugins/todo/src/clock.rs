//! The time of `@done(...)` and `@cancelled(...)`: Todo+ writes the local time in its timestamp
//! format, by default `YY-MM-DD HH:mm`. The component knows only UTC (`now-unix-seconds`), so the
//! caller names its offset (`utcOffset`, minutes east of UTC; the panel sends the computer's).

#[cfg(test)]
#[path = "clock_tests.rs"]
mod tests;

/// The furthest a time zone lies from UTC, in minutes (UTC+14:00).
pub const MAX_OFFSET_MINUTES: i64 = 14 * 60;

/// The timestamp formats offered (`timestampFormat`), each a format Todo+ users write.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Format {
    /// `YY-MM-DD HH:mm`, Todo+' default.
    #[default]
    TodoPlus,
    /// `YYYY-MM-DD HH:mm`.
    FullYear,
    /// `YY-MM-DD`.
    Date,
    /// `YYYY-MM-DD`.
    FullDate,
}

impl Format {
    /// The format of the settings value `id`.
    #[must_use]
    pub fn parse(id: &str) -> Option<Self> {
        match id {
            "todoPlus" => Some(Self::TodoPlus),
            "fullYear" => Some(Self::FullYear),
            "date" => Some(Self::Date),
            "fullDate" => Some(Self::FullDate),
            _ => None,
        }
    }
}

/// The civil date of `days` since 1970-01-01: `(year, month, day)` (Howard Hinnant's algorithm).
fn civil(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * shifted_month + 2) / 5 + 1;
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    // Both fit: a day is 1 to 31, a month 1 to 12.
    (
        year,
        u32::try_from(month).unwrap_or(1),
        u32::try_from(day).unwrap_or(1),
    )
}

/// `unix` seconds at `offset_minutes` east of UTC (held to +-14 h) in `format`.
#[must_use]
pub fn stamp(unix: u64, offset_minutes: i64, format: Format) -> String {
    let offset = offset_minutes.clamp(-MAX_OFFSET_MINUTES, MAX_OFFSET_MINUTES) * 60;
    let local = i64::try_from(unix).unwrap_or(i64::MAX - offset.abs()) + offset;
    let (year, month, day) = civil(local.div_euclid(86_400));
    let seconds = local.rem_euclid(86_400);
    let (hour, minute) = (seconds / 3600, seconds % 3600 / 60);
    let short_year = year.rem_euclid(100);
    match format {
        Format::TodoPlus => format!("{short_year:02}-{month:02}-{day:02} {hour:02}:{minute:02}"),
        Format::FullYear => format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}"),
        Format::Date => format!("{short_year:02}-{month:02}-{day:02}"),
        Format::FullDate => format!("{year:04}-{month:02}-{day:02}"),
    }
}
