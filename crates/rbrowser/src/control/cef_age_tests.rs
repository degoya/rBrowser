use super::*;

fn build(date: &str) -> CefBuild {
    CefBuild {
        version: "156.0.26+g8ea0c73+chromium-156.0.8078.88".to_owned(),
        date: date.to_owned(),
    }
}

fn day(date: &str) -> i64 {
    rb_diag::cef::days_from_date(date).expect("a date")
}

/// Updates not checked by themselves (`updates.check` off, RB-090-14): the notice speaks.
fn unchecked() -> UserSettings {
    let mut user = UserSettings::default();
    user.updates.check = rb_config::UpdateCheck::Off;
    user
}

#[test]
fn a_build_older_than_six_weeks_becomes_a_warning_with_a_settings_link() {
    let message =
        notification(&build("2026-09-01"), day("2026-11-10"), &unchecked()).expect("a notice");
    let CoreToShell::Notification(notice) = message else {
        panic!("a notification");
    };
    assert_eq!(notice.level, NotificationLevel::Warning);
    assert_eq!(notice.message_key, rb_diag::cef::CEF_AGE_NOTICE_KEY);
    assert_eq!(notice.args["days"], "70");
    assert_eq!(notice.args["date"], "2026-09-01");
    assert_eq!(
        notice.settings_key.as_deref(),
        Some(rb_diag::settings::KEY_CEF_AGE_WARNING)
    );
}

#[test]
fn a_young_build_or_the_switched_off_setting_raises_none() {
    let today = day("2026-11-10");
    assert_eq!(
        notification(&build("2026-11-07"), today, &unchecked()),
        None
    );
    let mut off = unchecked();
    off.updates.cef_age_warning = false;
    assert_eq!(notification(&build("2026-09-01"), today, &off), None);
}

/// RB-090-14: while updates are checked by themselves the notice stays silent; with the check
/// off, or in a build without self-update, it speaks.
#[test]
fn updates_checked_by_themselves_silence_the_notice() {
    let today = day("2026-11-10");
    let old = build("2026-09-01");
    let silent = notification(&old, today, &UserSettings::default());
    assert_eq!(silent.is_none(), crate::updates::SELF_UPDATE_BUILT);
    assert!(notification(&old, today, &unchecked()).is_some());
}

#[test]
fn the_embedded_build_is_the_pin_of_tool_versions_sh() {
    let pins = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scripts/lib/tool-versions.sh"),
    )
    .expect("the pin file");
    let embedded = embedded_build();
    assert!(
        pins.contains(&format!("RB_CEF_RELEASE_DATE=\"{}\"", embedded.date)),
        "{embedded:?}"
    );
    assert!(rb_diag::cef::days_from_date(&embedded.date).is_some());
    assert!(
        pins.contains(&format!("cef_binary_{}_windows64", embedded.version)),
        "{embedded:?}"
    );
}
