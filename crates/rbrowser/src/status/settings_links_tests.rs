use std::collections::BTreeMap;

use rb_ipc::{Notification, NotificationLevel};

use super::*;

fn notice(key: &str, args: &[(&str, &str)]) -> Notification {
    Notification {
        level: NotificationLevel::Error,
        message_key: key.to_owned(),
        args: args
            .iter()
            .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
            .collect::<BTreeMap<_, _>>(),
        settings_key: None,
    }
}

fn linked(mut notification: Notification) -> Option<String> {
    link_settings(&mut notification);
    notification.settings_key
}

#[test]
fn the_compiled_table_reads() {
    let links = links();
    assert!(links.len() > 20, "{} links", links.len());
    for (key, target) in links {
        assert!(rb_i18n::is_well_formed_key(key), "{key}");
        assert!(!target.trim().is_empty(), "{key} links to nothing");
    }
}

#[test]
fn a_notice_of_a_linked_text_links_to_its_setting() {
    assert_eq!(
        linked(notice("plugins.notice.new_failed", &[("plugins", "seo")])).as_deref(),
        Some("plugins.catalog")
    );
}

#[test]
fn a_notice_keeps_the_setting_its_code_named() {
    let mut notification = notice("portal.notice.off", &[]);
    notification.settings_key = Some("portal".to_owned());
    assert_eq!(linked(notification).as_deref(), Some("portal"));
}

#[test]
fn a_failure_whose_reason_is_a_linked_text_links_to_its_setting() {
    let reason = format!(
        "command failed: {}",
        rb_i18n::t("checks.export.error.no_project")
    );
    let failure = notice(
        "commands.notice.failed",
        &[("command", "Export findings"), ("error", &reason)],
    );
    assert_eq!(linked(failure).as_deref(), Some("checks.export.dir"));
}

#[test]
fn a_reason_with_its_placeholders_filled_still_counts() {
    let reason = rb_i18n::t_with(
        "engines.firefox.configuredMissing",
        &[("path", r"C:\Tools\firefox\firefox.exe")],
    );
    let failure = notice("commands.notice.failed", &[("error", &reason)]);
    assert_eq!(linked(failure).as_deref(), Some("engines.firefox.path"));
}

#[test]
fn a_reason_that_is_the_code_of_a_linked_text_counts() {
    let failure = notice(
        "commands.notice.failed",
        &[("error", "command failed: firstrun.error.off")],
    );
    assert_eq!(linked(failure).as_deref(), Some("ui.firstRunWizard"));
    let sentence = notice(
        "commands.notice.failed",
        &[("error", "see firstrun.error.off.")],
    );
    assert_eq!(linked(sentence).as_deref(), Some("ui.firstRunWizard"));
}

#[test]
fn a_longer_code_is_another_code() {
    for reason in [
        "command failed: firstrun.error.offline",
        "command failed: firstrun.error.off.more",
        "command failed: xfirstrun.error.off",
    ] {
        let failure = notice("commands.notice.failed", &[("error", reason)]);
        assert_eq!(linked(failure), None, "{reason}");
    }
}

#[test]
fn a_notice_that_names_no_setting_links_nowhere() {
    let failure = notice("commands.notice.failed", &[("error", "the disk is full")]);
    assert_eq!(linked(failure), None);
}

#[test]
fn the_literal_pieces_leave_the_placeholders_out() {
    assert_eq!(
        literal_pieces("The adb path in Settings > Engines > Android names no file: {path}"),
        vec!["The adb path in Settings > Engines > Android names no file: "]
    );
    assert_eq!(
        literal_pieces(
            "{count, plural, one {# value} other {# values}} missing: {names}. Set them."
        ),
        vec![" missing: ", ". Set them."]
    );
}

#[test]
fn a_short_text_never_matches_as_a_sentence() {
    // Fewer than MIN_LITERAL_CHARS of its own words: a template like "{a}: {b}".
    assert!(!holds_text("anything: at all", "{a}: {b}"));
    assert!(holds_text(
        "Firefox was not found at C:/ff (Settings > Engines > Firefox > Firefox path)",
        "Firefox was not found at {path} (Settings > Engines > Firefox > Firefox path)"
    ));
    assert!(!holds_text(
        "Firefox was found at C:/ff",
        "Firefox was not found at {path} (Settings > Engines > Firefox > Firefox path)"
    ));
}
