//! Values of the ordered controls - number, range, date, time, month, week, datetime-local -
//! inside or just outside their `min`, `max` and `step`. Each control's value maps to whole units
//! (days, minutes, months, weeks) or a number; a valid value comes with `min` and `max` as further
//! candidates, so the page script can fall back to one that fits a `step` this side ignores.

use crate::rng::Rng;

/// How a control's value maps to a number.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Scale {
    Number,
    /// Days since 1970-01-01.
    Date,
    /// Minutes since midnight.
    Time,
    /// Months since year 0.
    Month,
    /// Days of the week's Monday since 1970-01-01, a week a unit of 7.
    Week,
    /// Minutes since 1970-01-01T00:00.
    DateTime,
}

/// Days since 1970-01-01 of a civil date (H. Hinnant's algorithm).
#[must_use]
pub fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let m = i64::from(month);
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + i64::from(day) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The civil date of `days` since 1970-01-01.
#[must_use]
pub fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    (
        year,
        u32::try_from(month).unwrap_or(1),
        u32::try_from(day).unwrap_or(1),
    )
}

fn int(text: &str, digits: usize) -> Option<i64> {
    (text.len() == digits && text.bytes().all(|b| b.is_ascii_digit()))
        .then(|| text.parse().ok())
        .flatten()
}

fn parse_date(text: &str) -> Option<i64> {
    let mut parts = text.split('-');
    let year = int(parts.next()?, 4)?;
    let month = u32::try_from(int(parts.next()?, 2)?).ok()?;
    let day = u32::try_from(int(parts.next()?, 2)?).ok()?;
    if parts.next().is_some() || !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    let days = days_from_civil(year, month, day);
    (civil_from_days(days) == (year, month, day)).then_some(days)
}

fn parse_time(text: &str) -> Option<i64> {
    let hour = int(text.get(0..2)?, 2)?;
    let minute = int(text.get(3..5)?, 2)?;
    (text.as_bytes().get(2) == Some(&b':') && hour < 24 && minute < 60)
        .then_some(hour * 60 + minute)
}

/// The Monday of ISO week 1 of `year`, in days.
fn week_one(year: i64) -> i64 {
    let jan4 = days_from_civil(year, 1, 4);
    // 1970-01-01 was a Thursday: weekday 0 is Monday.
    let weekday = (jan4 + 3).rem_euclid(7);
    jan4 - weekday
}

fn parse_week(text: &str) -> Option<i64> {
    let (year, week) = text.split_once("-W")?;
    let year = int(year, 4)?;
    let week = int(week, 2)?;
    let weeks_in_year = (week_one(year + 1) - week_one(year)) / 7;
    (1..=weeks_in_year)
        .contains(&week)
        .then(|| week_one(year) + (week - 1) * 7)
}

fn format_week(monday: i64) -> String {
    let (year, _, _) = civil_from_days(monday + 3);
    let week = (monday - week_one(year)) / 7 + 1;
    format!("{year:04}-W{week:02}")
}

impl Scale {
    /// The value `text` in units; none when it is no such value.
    #[must_use]
    pub fn parse(self, text: &str) -> Option<i64> {
        match self {
            Scale::Number => None,
            Scale::Date => parse_date(text),
            Scale::Time => parse_time(text),
            Scale::Month => {
                let (year, month) = text.split_once('-')?;
                let month = int(month, 2)?;
                (1..=12)
                    .contains(&month)
                    .then(|| int(year, 4).map(|year| year * 12 + month - 1))
                    .flatten()
            }
            Scale::Week => parse_week(text),
            Scale::DateTime => {
                let (date, time) = text.split_once('T')?;
                Some(parse_date(date)? * 1440 + parse_time(time)?)
            }
        }
    }

    /// The value of `units` as the control writes it.
    #[must_use]
    pub fn format(self, units: i64) -> String {
        match self {
            Scale::Number => units.to_string(),
            Scale::Date => {
                let (year, month, day) = civil_from_days(units);
                format!("{year:04}-{month:02}-{day:02}")
            }
            Scale::Time => format!("{:02}:{:02}", units.div_euclid(60), units.rem_euclid(60)),
            Scale::Month => format!(
                "{:04}-{:02}",
                units.div_euclid(12),
                units.rem_euclid(12) + 1
            ),
            Scale::Week => format_week(units),
            Scale::DateTime => format!(
                "{}T{}",
                Scale::Date.format(units.div_euclid(1440)),
                Scale::Time.format(units.rem_euclid(1440))
            ),
        }
    }

    /// One unit of the scale (a week is seven days).
    fn unit(self) -> i64 {
        if self == Scale::Week { 7 } else { 1 }
    }

    /// The units of a value without bounds, and how far it may wander.
    fn default_around(self) -> (i64, i64) {
        let june = days_from_civil(2026, 6, 15);
        match self {
            Scale::Number => (50, 49),
            Scale::Date => (june, 180),
            Scale::Time => (12 * 60 + 30, 4 * 60),
            Scale::Month => (2026 * 12 + 5, 6),
            Scale::Week => (week_one(2026) + 25 * 7, 20),
            Scale::DateTime => (june * 1440 + 12 * 60 + 30, 180 * 1440),
        }
    }
}

/// The decimals of a number written as `text`.
#[must_use]
pub fn decimals(text: &str) -> usize {
    text.split_once('.')
        .map_or(0, |(_, fraction)| fraction.len())
}

/// `value` rounded to `places` decimals, without trailing zeros.
#[must_use]
pub fn number_text(value: f64, places: usize) -> String {
    let text = format!("{value:.places$}");
    let text = if text.contains('.') {
        text.trim_end_matches('0').trim_end_matches('.').to_owned()
    } else {
        text
    };
    if text == "-0" { "0".to_owned() } else { text }
}

fn number(text: &str) -> Option<f64> {
    text.trim().parse::<f64>().ok().filter(|n| n.is_finite())
}

fn step_of(step: &str) -> Option<f64> {
    number(step).filter(|n| *n > 0.0)
}

fn valid_number(min: &str, max: &str, step: &str, rng: &mut Rng) -> Vec<String> {
    let lo = number(min);
    let hi = number(max);
    let explicit = step_of(step);
    let stride = explicit.unwrap_or(1.0);
    let places = decimals(step).max(decimals(min));
    let base = lo.unwrap_or(0.0);
    let (from, to) = match (lo, hi) {
        (Some(lo), Some(hi)) => (lo, hi),
        (Some(lo), None) => (lo, lo + stride * 20.0),
        (None, Some(hi)) => (hi - stride * 20.0, hi),
        (None, None) => (1.0, 100.0),
    };
    let first = ((from - base) / stride).ceil();
    let last = ((to - base) / stride).floor();
    let mut out = Vec::new();
    if last >= first {
        let span = (last - first).min(1.0e9) as u64;
        let k = first + rng.below(span + 1) as f64;
        out.push(number_text(base + k * stride, places));
    }
    out.extend(lo.map(|n| number_text(n, places)));
    out.extend(hi.map(|n| number_text(n, places)));
    if out.is_empty() {
        out.push(number_text(from, places));
    }
    out
}

/// A value inside `min` and `max` first, then `min` and `max` themselves.
#[must_use]
pub fn valid_candidates(
    scale: Scale,
    min: &str,
    max: &str,
    step: &str,
    rng: &mut Rng,
) -> Vec<String> {
    if scale == Scale::Number {
        return valid_number(min, max, step, rng);
    }
    let lo = scale.parse(min);
    let hi = scale.parse(max);
    let unit = scale.unit();
    let (center, spread) = scale.default_around();
    let (from, to) = match (lo, hi) {
        (Some(lo), Some(hi)) => (lo, hi.max(lo)),
        (Some(lo), None) => (lo, lo + 30 * unit),
        (None, Some(hi)) => (hi - 30 * unit, hi),
        (None, None) => (center - spread * unit, center + spread * unit),
    };
    let steps = (to - from) / unit;
    let mut out = vec![scale.format(from + rng.between(0, steps) * unit)];
    out.extend(lo.map(|units| scale.format(units)));
    out.extend(hi.map(|units| scale.format(units)));
    out
}

/// A value one unit (or one step) below `min`, else above `max`; none without either.
#[must_use]
pub fn out_of_bounds(scale: Scale, min: &str, max: &str, step: &str) -> Option<String> {
    if scale == Scale::Number {
        let stride = step_of(step).unwrap_or(1.0);
        let places = decimals(step).max(decimals(min)).max(decimals(max));
        return number(min)
            .map(|lo| number_text(lo - stride, places))
            .or_else(|| number(max).map(|hi| number_text(hi + stride, places)));
    }
    let unit = scale.unit();
    scale
        .parse(min)
        .map(|lo| scale.format(lo - unit))
        .or_else(|| scale.parse(max).map(|hi| scale.format(hi + unit)))
}

/// A number half a step off the step base (`min`, else 0); none without a step.
#[must_use]
pub fn off_step(min: &str, max: &str, step: &str) -> Option<String> {
    let _ = max;
    let stride = step_of(step)?;
    let base = number(min).unwrap_or(0.0);
    let places = decimals(step).max(decimals(min)) + 1;
    Some(number_text(base + stride / 2.0, places))
}

#[cfg(test)]
#[path = "bounds_tests.rs"]
mod tests;
