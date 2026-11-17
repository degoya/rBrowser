//! The CEF side of diagnostics, as data for the CEF app (RB-010-04): the age check of the
//! embedded CEF build, the `crash_reporter.cfg` that keeps Crashpad's dumps local, and the
//! severity of CEF's own log file.
//!
//! No date crate: a build date is `YYYY-MM-DD` (what `build.rs` embeds), turned into days since
//! the Unix epoch with the civil-calendar arithmetic below.

use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::settings::{KEY_CEF_AGE_WARNING, LogLevel};

/// A CEF build older than this many days raises the notice: six weeks, about one Chromium
/// release cycle.
pub const CEF_AGE_LIMIT_DAYS: i64 = 42;

/// The i18n key of the notice; placeholders `{version}`, `{date}`, `{days}`.
pub const CEF_AGE_NOTICE_KEY: &str = "diagnostics.cefAge.notice";

/// The embedded CEF build: its version and its build date (`YYYY-MM-DD`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CefBuild {
    /// The CEF version, `156.4.0+g...+chromium-156.0.x.y`.
    pub version: String,
    /// The build date, `YYYY-MM-DD` (a longer RFC 3339 time is cut to its date).
    pub date: String,
}

/// The shell's notice about an old CEF: the fields of `rb_ipc::Notification` (level `warning`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CefAgeNotice {
    /// [`CEF_AGE_NOTICE_KEY`].
    pub message_key: String,
    /// `version`, `date` and `days` for the text's placeholders.
    pub args: BTreeMap<String, String>,
    /// The setting that switches the notice off ([`KEY_CEF_AGE_WARNING`]).
    pub settings_key: Option<String>,
}

/// The notice when `build` is older than [`CEF_AGE_LIMIT_DAYS`] on day `today` (days since the
/// epoch), the notice is `enabled` (`updates.cefAgeWarning`) and no self-update is active;
/// `None` otherwise, and when the build date cannot be read.
pub fn cef_age_notice(
    build: &CefBuild,
    today: i64,
    enabled: bool,
    update_active: bool,
) -> Option<CefAgeNotice> {
    if !enabled || update_active {
        return None;
    }
    let built = days_from_date(&build.date)?;
    let age = today - built;
    if age <= CEF_AGE_LIMIT_DAYS {
        return None;
    }
    let args = BTreeMap::from([
        ("version".to_owned(), build.version.clone()),
        (
            "date".to_owned(),
            build.date.get(..10).unwrap_or_default().to_owned(),
        ),
        ("days".to_owned(), age.to_string()),
    ]);
    Some(CefAgeNotice {
        message_key: CEF_AGE_NOTICE_KEY.to_owned(),
        args,
        settings_key: Some(KEY_CEF_AGE_WARNING.to_owned()),
    })
}

/// The days since the Unix epoch of `YYYY-MM-DD` (the first ten characters of `text`); `None`
/// for anything else, an impossible day included.
pub fn days_from_date(text: &str) -> Option<i64> {
    let date = text.get(..10)?;
    let mut parts = date.split('-');
    let (year, month, day) = (parts.next()?, parts.next()?, parts.next()?);
    if year.len() != 4 || month.len() != 2 || day.len() != 2 {
        return None;
    }
    let digits = |part: &str| {
        part.bytes()
            .all(|byte| byte.is_ascii_digit())
            .then(|| part.parse::<i64>().ok())
            .flatten()
    };
    let (year, month, day) = (digits(year)?, digits(month)?, digits(day)?);
    if !(1..=12).contains(&month) || day < 1 || day > days_in_month(year, month) {
        return None;
    }
    // Howard Hinnant's days_from_civil.
    let shifted = if month <= 2 { year - 1 } else { year };
    let era = shifted.div_euclid(400);
    let year_of_era = shifted - era * 400;
    let month_index = (month + 9) % 12;
    let day_of_year = (153 * month_index + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    Some(era * 146_097 + day_of_era - 719_468)
}

/// The civil date (year, month, day) of `days` since the Unix epoch; Howard Hinnant's
/// civil_from_days.
pub fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    (
        year,
        u32::try_from(month).unwrap_or(1),
        u32::try_from(day).unwrap_or(1),
    )
}

fn days_in_month(year: i64, month: i64) -> i64 {
    let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    match month {
        2 if leap => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// The days since the Unix epoch at `time` (0 before it).
pub fn days_since_epoch(time: SystemTime) -> i64 {
    time.duration_since(UNIX_EPOCH).map_or(0, |since| {
        i64::try_from(since.as_secs() / 86_400).unwrap_or(i64::MAX)
    })
}

/// The content of `crash_reporter.cfg`, which the package puts beside `rbrowser.exe`: Crashpad
/// writes dumps to `%LOCALAPPDATA%\rBrowser\User Data\Crashpad` and, without a `ServerURL`,
/// never uploads them.
pub fn crash_reporter_cfg(product_version: &str) -> String {
    format!(
        "# rBrowser: crash dumps stay on this machine. No ServerURL - nothing is uploaded.\n\
         [Config]\n\
         ProductName=rBrowser\n\
         ProductVersion={product_version}\n\
         AppName=rBrowser\n\
         RateLimitEnabled=true\n\
         MaxUploadsPerDay=0\n\
         MaxDatabaseSizeInMb=50\n\
         MaxDatabaseAgeInDays=30\n"
    )
}

/// CEF's `cef_log_severity_t`, for `CefSettings.log_severity`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CefLogSeverity {
    /// `LOGSEVERITY_VERBOSE` (= debug).
    Verbose,
    /// `LOGSEVERITY_INFO`.
    Info,
    /// `LOGSEVERITY_WARNING`.
    Warning,
    /// `LOGSEVERITY_ERROR`.
    Error,
}

impl CefLogSeverity {
    /// The raw value of `cef_log_severity_t`.
    pub fn raw(self) -> i32 {
        match self {
            CefLogSeverity::Verbose => 1,
            CefLogSeverity::Info => 2,
            CefLogSeverity::Warning => 3,
            CefLogSeverity::Error => 4,
        }
    }
}

impl From<LogLevel> for CefLogSeverity {
    /// One step quieter than ours: Chromium's info level is a flood.
    fn from(level: LogLevel) -> Self {
        match level {
            LogLevel::Trace => CefLogSeverity::Verbose,
            LogLevel::Debug => CefLogSeverity::Info,
            LogLevel::Info | LogLevel::Warn => CefLogSeverity::Warning,
            LogLevel::Error => CefLogSeverity::Error,
        }
    }
}

#[cfg(test)]
#[path = "cef_tests.rs"]
mod tests;
