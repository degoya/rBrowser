//! The text forms of schema.org's data types the validation checks: ISO 8601 dates, times and
//! date-times as schema.org and Google Search read them, and the two booleans.

fn digits(text: &str, count: usize) -> bool {
    text.len() == count && text.bytes().all(|byte| byte.is_ascii_digit())
}

/// `YYYY`, `YYYY-MM` or `YYYY-MM-DD`, with a month of 1 to 12 and a day of 1 to 31.
#[must_use]
pub fn is_date(text: &str) -> bool {
    let parts: Vec<&str> = text.split('-').collect();
    let in_range =
        |part: &str, high: u32| part.parse::<u32>().is_ok_and(|n| (1..=high).contains(&n));
    match parts.as_slice() {
        [year] => digits(year, 4),
        [year, month] => digits(year, 4) && digits(month, 2) && in_range(month, 12),
        [year, month, day] => {
            digits(year, 4)
                && digits(month, 2)
                && digits(day, 2)
                && in_range(month, 12)
                && in_range(day, 31)
        }
        _ => false,
    }
}

/// A time zone: `Z`, `+HH:MM`, `-HH:MM` or `+HHMM`.
fn is_zone(text: &str) -> bool {
    if text == "Z" {
        return true;
    }
    let Some(rest) = text.strip_prefix('+').or_else(|| text.strip_prefix('-')) else {
        return false;
    };
    let rest = rest.replace(':', "");
    digits(&rest, 4) || digits(&rest, 2)
}

/// `HH:MM`, `HH:MM:SS` or `HH:MM:SS.fff`, with an optional time zone.
#[must_use]
pub fn is_time(text: &str) -> bool {
    let split = text
        .find(|c: char| c == 'Z' || c == '+' || c == '-')
        .unwrap_or(text.len());
    let (clock, zone) = text.split_at(split);
    if !zone.is_empty() && !is_zone(zone) {
        return false;
    }
    let (clock, fraction) = clock.split_once('.').unwrap_or((clock, "0"));
    if fraction.is_empty() || !fraction.bytes().all(|byte| byte.is_ascii_digit()) {
        return false;
    }
    let parts: Vec<&str> = clock.split(':').collect();
    let below =
        |part: &str, limit: u32| digits(part, 2) && part.parse::<u32>().is_ok_and(|n| n < limit);
    match parts.as_slice() {
        [hours, minutes] => below(hours, 25) && below(minutes, 60),
        [hours, minutes, seconds] => below(hours, 25) && below(minutes, 60) && below(seconds, 61),
        _ => false,
    }
}

/// A full date (`YYYY-MM-DD`), alone or with a time after `T` or a space.
#[must_use]
pub fn is_date_time(text: &str) -> bool {
    match text.split_once(['T', ' ']) {
        Some((date, time)) => date.len() == 10 && is_date(date) && is_time(time),
        None => text.len() == 10 && is_date(text),
    }
}

/// `true` or `false`, in any case, or schema.org's `True` and `False` (also as IRIs).
#[must_use]
pub fn is_boolean(text: &str) -> bool {
    let name = schema_org::short_name(text).unwrap_or(text);
    name.eq_ignore_ascii_case("true") || name.eq_ignore_ascii_case("false")
}

#[cfg(test)]
#[path = "formats_tests.rs"]
mod tests;
