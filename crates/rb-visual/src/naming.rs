//! The names of the screenshot files: the path pattern (`capture.pathPattern`, e.g.
//! `{date}/{env}/{breakpoint}`) expanded per view into a relative path below the output folder.
//! Every segment is sanitised, so a breakpoint label or a pattern can never leave the folder or
//! name a file Windows refuses.

use std::path::PathBuf;

use crate::VisualError;

/// The pattern when `capture.pathPattern` is not set: one folder per run, one file per
/// breakpoint.
pub const DEFAULT_PATTERN: &str = "{date}/{breakpoint}";
/// The longest segment a pattern expands to.
pub const MAX_SEGMENT: usize = 80;
/// The placeholders a pattern may use.
pub const PLACEHOLDERS: [&str; 10] = [
    "date",
    "day",
    "time",
    "env",
    "breakpoint",
    "view",
    "mode",
    "width",
    "height",
    "dpr",
];
/// The names Windows reserves for devices, in any case and with any extension.
const RESERVED_NAMES: [&str; 22] = [
    "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
    "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
];

/// A point in time as the file names show it: the UTC calendar date and time of day.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stamp {
    year: i64,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
    second: u32,
}

impl Stamp {
    /// The stamp of `secs` seconds after the Unix epoch, in UTC.
    pub fn from_unix(secs: u64) -> Self {
        let days = i64::try_from(secs / 86_400).unwrap_or(i64::MAX / 2);
        let rest = u32::try_from(secs % 86_400).unwrap_or(0);
        let (year, month, day) = civil_from_days(days);
        Self {
            year,
            month,
            day,
            hour: rest / 3600,
            minute: rest % 3600 / 60,
            second: rest % 60,
        }
    }

    /// The stamp of now.
    pub fn now() -> Self {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_secs());
        Self::from_unix(secs)
    }

    /// `2026-11-10`.
    pub fn day(&self) -> String {
        format!("{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }

    /// `14-03-07`.
    pub fn time(&self) -> String {
        format!("{:02}-{:02}-{:02}", self.hour, self.minute, self.second)
    }

    /// `2026-11-10_14-03-07`: `{date}`, the folder of one run.
    pub fn date_time(&self) -> String {
        format!("{}_{}", self.day(), self.time())
    }
}

/// Howard Hinnant's `civil_from_days`: the proleptic Gregorian date of a day count.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
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

/// What the placeholders of one file stand for.
#[derive(Clone, Debug, PartialEq)]
pub struct PathVars {
    /// `{date}`, `{day}`, `{time}`: when the run started (one stamp for every view of a run).
    pub stamp: Stamp,
    /// `{env}`: the environment id; `default` when none is chosen.
    pub env: Option<String>,
    /// `{breakpoint}`: the breakpoint id, or the size of a free size (`500x800`).
    pub breakpoint: String,
    /// `{view}`: the view id.
    pub view: u32,
    /// `{mode}`: `viewport`, `fullPage`, `element` or `overview`.
    pub mode: String,
    /// `{width}`: viewport width in CSS px.
    pub width: u32,
    /// `{height}`: viewport height in CSS px.
    pub height: u32,
    /// `{dpr}`: the device pixel ratio, as `3x`; empty for the display's own.
    pub dpr: f64,
}

impl PathVars {
    fn value(&self, name: &str) -> Option<String> {
        Some(match name {
            "date" => self.stamp.date_time(),
            "day" => self.stamp.day(),
            "time" => self.stamp.time(),
            "env" => self.env.clone().unwrap_or_else(|| "default".to_owned()),
            "breakpoint" => self.breakpoint.clone(),
            "view" => self.view.to_string(),
            "mode" => self.mode.clone(),
            "width" => self.width.to_string(),
            "height" => self.height.to_string(),
            "dpr" => dpr_text(self.dpr),
            _ => return None,
        })
    }
}

/// Checks `pattern` without expanding it: braces closed, every placeholder known, not empty.
pub fn validate_pattern(pattern: &str) -> Result<(), VisualError> {
    if pattern.trim().is_empty() {
        return Err(VisualError::Pattern("the pattern is empty".to_owned()));
    }
    let mut rest = pattern;
    while let Some(open) = rest.find('{') {
        let after = &rest[open + 1..];
        let close = after
            .find('}')
            .ok_or_else(|| VisualError::Pattern(format!("an unclosed brace in {pattern}")))?;
        let name = &after[..close];
        if !PLACEHOLDERS.contains(&name) {
            return Err(VisualError::Pattern(format!(
                "unknown placeholder {{{name}}}"
            )));
        }
        rest = &after[close + 1..];
    }
    Ok(())
}

/// The relative path (without extension) `pattern` gives for `vars`: `/` separates folders,
/// each segment is expanded and then sanitised ([`sanitize_segment`]); empty segments are
/// dropped.
pub fn expand(pattern: &str, vars: &PathVars) -> Result<PathBuf, VisualError> {
    validate_pattern(pattern)?;
    let mut path = PathBuf::new();
    for segment in pattern.split(['/', '\\']) {
        if segment.is_empty() {
            continue;
        }
        let mut expanded = String::new();
        let mut rest = segment;
        while let Some(open) = rest.find('{') {
            expanded.push_str(&rest[..open]);
            let after = &rest[open + 1..];
            let close = after.find('}').unwrap_or(after.len());
            let name = &after[..close];
            let value = vars
                .value(name)
                .ok_or_else(|| VisualError::Pattern(format!("unknown placeholder {{{name}}}")))?;
            expanded.push_str(&value);
            rest = after.get(close + 1..).unwrap_or_default();
        }
        expanded.push_str(rest);
        path.push(sanitize_segment(&expanded));
    }
    if path.as_os_str().is_empty() {
        return Err(VisualError::Pattern("the pattern names no file".to_owned()));
    }
    Ok(path)
}

/// One path segment that is safe on Windows and Linux: ASCII letters, digits, `-`, `_`, `.`,
/// `@` and `+`; every run of other characters becomes one `-`. No leading or trailing dot or
/// dash, no device name (`CON`, `LPT1`), at most [`MAX_SEGMENT`] characters, never empty
/// (`_`).
pub fn sanitize_segment(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut replaced = false;
    for c in text.chars() {
        if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '@' | '+') {
            out.push(c);
            replaced = false;
        } else if !replaced {
            out.push('-');
            replaced = true;
        }
    }
    let trimmed = out.trim_matches(|c| c == '.' || c == '-');
    let mut segment: String = trimmed.chars().take(MAX_SEGMENT).collect();
    if segment.is_empty() {
        return "_".to_owned();
    }
    if is_device_name(&segment) {
        segment.insert(0, '_');
        segment.truncate(MAX_SEGMENT);
    }
    segment
}

/// Whether `name` is a name Windows reserves for a device (`CON`, `lpt1.png`): its part before
/// the first dot, in any case.
pub fn is_device_name(name: &str) -> bool {
    let stem = name.split('.').next().unwrap_or_default();
    RESERVED_NAMES
        .iter()
        .any(|reserved| stem.eq_ignore_ascii_case(reserved))
}

/// `3x`, `1.5x`, `2.625x`; empty for `0` (the display's own).
pub fn dpr_text(dpr: f64) -> String {
    if dpr <= 0.0 || !dpr.is_finite() {
        return String::new();
    }
    let text = format!("{dpr:.3}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    format!("{text}x")
}

/// The size line of a view: `393 x 852 @3x` (with a multiplication sign), without the `@` part
/// for the display's own DPR.
pub fn size_text(width: u32, height: u32, dpr: f64) -> String {
    let dpr = dpr_text(dpr);
    if dpr.is_empty() {
        format!("{width} \u{d7} {height}")
    } else {
        format!("{width} \u{d7} {height} @{dpr}")
    }
}

#[cfg(test)]
#[path = "naming_tests.rs"]
mod tests;
