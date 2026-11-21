//! An HTTP date (`Date`, a cookie's `Expires`) as Unix seconds, read loosely as RFC 6265, 5.1.1
//! reads a cookie date: the tokens in any order - a time `hh:mm:ss`, a day, a month's name, a
//! year (two digits: 70-99 are 19xx, 00-69 20xx) - so IMF-fixdate, RFC 850 and asctime all read.
//! A foreign server's text: at most [`MAX_TOKENS`] tokens are looked at; nothing indexes into it.

/// The most tokens of one date looked at.
pub const MAX_TOKENS: usize = 16;

const MONTHS: [&str; 12] = [
    "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
];

fn number(token: &str, digits: std::ops::RangeInclusive<usize>) -> Option<i64> {
    (digits.contains(&token.len()) && token.bytes().all(|byte| byte.is_ascii_digit()))
        .then(|| token.parse().ok())
        .flatten()
}

fn time(token: &str) -> Option<i64> {
    let mut parts = token.split(':');
    let hour = number(parts.next()?, 1..=2)?;
    let minute = number(parts.next()?, 1..=2)?;
    let second = number(parts.next()?, 1..=2)?;
    (parts.next().is_none() && hour < 24 && minute < 60 && second < 60)
        .then_some(hour * 3600 + minute * 60 + second)
}

/// Days since 1970-01-01 of a civil date (H. Hinnant's `days_from_civil`).
fn days(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let of_era = year - era * 400;
    let of_year = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let of_cycle = of_era * 365 + of_era / 4 - of_era / 100 + of_year;
    era * 146_097 + of_cycle - 719_468
}

/// The Unix seconds of `text`; `None` when it is no date.
#[must_use]
pub fn parse(text: &str) -> Option<i64> {
    let (mut clock, mut day, mut month, mut year) = (None, None, None, None);
    let tokens = text
        .split(|c: char| c == ' ' || c == ',' || c == '-' || c == '\t')
        .filter(|token| !token.is_empty())
        .take(MAX_TOKENS);
    for token in tokens {
        if clock.is_none() && token.contains(':') {
            clock = Some(time(token)?);
        } else if day.is_none()
            && let Some(value) = number(token, 1..=2)
        {
            day = Some(value);
        } else if month.is_none()
            && let Some(at) = token.get(..3).and_then(|start| {
                MONTHS
                    .iter()
                    .position(|name| start.eq_ignore_ascii_case(name))
            })
        {
            month = Some(i64::try_from(at).ok()? + 1);
        } else if year.is_none()
            && let Some(value) = number(token, 2..=4)
        {
            year = Some(match value {
                0..=69 if token.len() == 2 => value + 2000,
                70..=99 if token.len() == 2 => value + 1900,
                _ => value,
            });
        }
    }
    let (clock, day, month, year) = (clock?, day?, month?, year?);
    if !(1..=31).contains(&day) || !(1601..=9999).contains(&year) {
        return None;
    }
    Some(days(year, month, day) * 86_400 + clock)
}

#[cfg(test)]
#[path = "date_tests.rs"]
mod tests;
