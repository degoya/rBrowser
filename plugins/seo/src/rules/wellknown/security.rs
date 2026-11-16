//! `security.txt` (RFC 9116): its fields - a signed file's text between the PGP header and the
//! signature, dash-escaped lines unescaped - and the RFC 3339 time of `Expires`.

/// The fields the rules read.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Fields {
    pub contact: Vec<String>,
    pub expires: Vec<String>,
}

/// The lines of `text` that carry fields: a cleartext-signed file's signed part.
fn signed_part(text: &str) -> Vec<&str> {
    if !text.contains("-----BEGIN PGP SIGNED MESSAGE-----") {
        return text.lines().collect();
    }
    text.lines()
        .skip_while(|line| !line.starts_with("-----BEGIN PGP SIGNED MESSAGE-----"))
        .skip(1)
        .skip_while(|line| !line.trim().is_empty())
        .take_while(|line| !line.starts_with("-----BEGIN PGP SIGNATURE-----"))
        .map(|line| line.strip_prefix("- ").unwrap_or(line))
        .collect()
}

/// The fields of `text`; comments and unknown fields skipped.
#[must_use]
pub fn fields(text: &str) -> Fields {
    let mut fields = Fields::default();
    for line in signed_part(text) {
        let line = line.trim();
        if line.starts_with('#') {
            continue;
        }
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.trim().to_owned();
        match name.trim().to_ascii_lowercase().as_str() {
            "contact" => fields.contact.push(value),
            "expires" => fields.expires.push(value),
            _ => {}
        }
    }
    fields
}

/// Days since 1970-01-01 of a civil date (Howard Hinnant's `days_from_civil`).
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let month_index = (month + 9) % 12;
    let day_of_year = (153 * month_index + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

fn digits(text: &str, from: usize, count: usize) -> Option<i64> {
    let part = text.get(from..from + count)?;
    part.bytes()
        .all(|byte| byte.is_ascii_digit())
        .then(|| part.parse().ok())
        .flatten()
}

/// `text` as RFC 3339 (`2027-01-31T23:59:59Z`, fractions and an offset allowed) in seconds
/// since the epoch; `None` when it is no such time.
#[must_use]
pub fn parse_time(text: &str) -> Option<i64> {
    let text = text.trim();
    let bytes = text.as_bytes();
    if bytes.len() < 20
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || !matches!(bytes[10], b'T' | b't' | b' ')
        || bytes[13] != b':'
        || bytes[16] != b':'
    {
        return None;
    }
    let (year, month, day) = (
        digits(text, 0, 4)?,
        digits(text, 5, 2)?,
        digits(text, 8, 2)?,
    );
    let (hour, minute, second) = (
        digits(text, 11, 2)?,
        digits(text, 14, 2)?,
        digits(text, 17, 2)?,
    );
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days_in_month = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return None,
    };
    if !(1..=days_in_month).contains(&day) || hour > 23 || minute > 59 || second > 60 {
        return None;
    }
    let mut rest = &text[19..];
    if let Some(fraction) = rest.strip_prefix('.') {
        let end = fraction
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(fraction.len());
        if end == 0 {
            return None;
        }
        rest = &fraction[end..];
    }
    let offset = match rest {
        "Z" | "z" => 0,
        _ if rest.len() == 6 && rest.as_bytes()[3] == b':' => {
            let sign = match rest.as_bytes()[0] {
                b'+' => 1,
                b'-' => -1,
                _ => return None,
            };
            let (hours, minutes) = (digits(rest, 1, 2)?, digits(rest, 4, 2)?);
            if hours > 23 || minutes > 59 {
                return None;
            }
            sign * (hours * 3600 + minutes * 60)
        }
        _ => return None,
    };
    let days = days_from_civil(year, month, day);
    Some(days * 86_400 + hour * 3600 + minute * 60 + second - offset)
}

#[cfg(test)]
#[path = "security_tests.rs"]
mod tests;
