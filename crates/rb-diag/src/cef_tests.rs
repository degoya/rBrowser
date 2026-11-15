use std::time::{Duration, UNIX_EPOCH};

use super::*;

fn build(date: &str) -> CefBuild {
    CefBuild {
        version: "156.4.0+chromium-156.0.7000.1".to_owned(),
        date: date.to_owned(),
    }
}

#[test]
fn civil_dates_count_days_from_the_epoch() {
    assert_eq!(days_from_date("1970-01-01"), Some(0));
    assert_eq!(days_from_date("2000-03-01"), Some(11_017));
    assert_eq!(days_from_date("2026-11-10"), Some(20_767));
    assert_eq!(days_from_date("2026-11-10T08:15:00Z"), Some(20_767));
    assert_eq!(days_from_date("2024-02-29"), Some(19_782));
}

#[test]
fn days_turn_back_into_civil_dates() {
    assert_eq!(civil_from_days(0), (1970, 1, 1));
    assert_eq!(civil_from_days(20_767), (2026, 11, 10));
    assert_eq!(civil_from_days(19_782), (2024, 2, 29));
    for days in [-1_000, 0, 11_016, 11_017, 20_766, 40_000] {
        let (year, month, day) = civil_from_days(days);
        let text = format!("{year:04}-{month:02}-{day:02}");
        assert_eq!(days_from_date(&text), Some(days), "{text}");
    }
}

#[test]
fn malformed_dates_are_refused() {
    for text in [
        "",
        "2026-13-01",
        "2026-02-30",
        "2025-02-29",
        "26-11-10",
        "2026/11/10",
    ] {
        assert_eq!(days_from_date(text), None, "{text}");
    }
}

#[test]
fn days_since_epoch_reads_the_clock() {
    let time = UNIX_EPOCH + Duration::from_secs(20_767 * 86_400 + 3_600);
    assert_eq!(days_since_epoch(time), 20_767);
}

#[test]
fn a_build_43_days_old_raises_the_notice() {
    let today = days_from_date("2026-11-10").expect("date");
    let notice = cef_age_notice(&build("2026-09-28"), today, true, false).expect("notice");
    assert_eq!(notice.message_key, CEF_AGE_NOTICE_KEY);
    assert_eq!(notice.args.get("days").map(String::as_str), Some("43"));
    assert_eq!(
        notice.args.get("date").map(String::as_str),
        Some("2026-09-28")
    );
    assert_eq!(
        notice.settings_key.as_deref(),
        Some(crate::settings::KEY_CEF_AGE_WARNING)
    );
}

#[test]
fn a_build_of_exactly_six_weeks_raises_none() {
    let today = days_from_date("2026-11-10").expect("date");
    assert_eq!(
        cef_age_notice(&build("2026-09-29"), today, true, false),
        None
    );
}

#[test]
fn the_notice_stays_off_when_disabled_or_when_updates_run() {
    let today = days_from_date("2026-11-10").expect("date");
    assert_eq!(
        cef_age_notice(&build("2026-01-01"), today, false, false),
        None
    );
    assert_eq!(
        cef_age_notice(&build("2026-01-01"), today, true, true),
        None
    );
}

#[test]
fn an_unreadable_build_date_raises_none() {
    let today = days_from_date("2026-11-10").expect("date");
    assert_eq!(cef_age_notice(&build("unknown"), today, true, false), None);
}

#[test]
fn the_crash_reporter_config_never_uploads() {
    let cfg = crash_reporter_cfg("0.1.0");
    assert!(cfg.starts_with("# "));
    assert!(cfg.contains("[Config]\n"));
    assert!(cfg.contains("ProductName=rBrowser\n"));
    assert!(cfg.contains("ProductVersion=0.1.0\n"));
    assert!(cfg.contains("AppName=rBrowser\n"));
    assert!(cfg.contains("RateLimitEnabled=true\n"));
    assert!(cfg.contains("MaxUploadsPerDay=0\n"));
    assert!(
        !cfg.lines().any(|line| line.starts_with("ServerURL")),
        "no ServerURL line: dumps stay local"
    );
}

#[test]
fn cef_log_severity_follows_the_level() {
    use crate::settings::LogLevel;
    assert_eq!(
        CefLogSeverity::from(LogLevel::Trace),
        CefLogSeverity::Verbose
    );
    assert_eq!(CefLogSeverity::from(LogLevel::Debug), CefLogSeverity::Info);
    assert_eq!(
        CefLogSeverity::from(LogLevel::Info),
        CefLogSeverity::Warning
    );
    assert_eq!(
        CefLogSeverity::from(LogLevel::Warn),
        CefLogSeverity::Warning
    );
    assert_eq!(CefLogSeverity::from(LogLevel::Error), CefLogSeverity::Error);
    assert_eq!(CefLogSeverity::Warning.raw(), 3);
}
