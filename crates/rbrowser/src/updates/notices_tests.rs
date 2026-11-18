use rb_update::{Artifact, Channel, DateTime, Utc};

use super::*;

fn offered(artifact: bool) -> Offer {
    Offer {
        version: "0.9.0-beta.3".to_owned(),
        channel: Channel::Beta,
        released_at: DateTime::<Utc>::from_timestamp(1_790_000_000, 0).expect("time"),
        notes: "- Self-update\n\n- Plugins leave the package\n".to_owned(),
        artifact: artifact.then(|| Artifact {
            platform: "windows".to_owned(),
            arch: "x86_64".to_owned(),
            kind: "zip".to_owned(),
            url: "https://github.com/degoya/rBrowser/releases/download/v0.9.0-beta.3/x.zip"
                .to_owned(),
            sha256: "ab".repeat(32),
            size: 209_400_001,
        }),
    }
}

#[test]
fn a_portable_installation_is_offered_install_and_restart_with_version_size_and_notes() {
    let (notice, action) = offer(&offered(true), InstallKind::Portable);
    assert_eq!(notice.message_key, "update.notice.available");
    assert_eq!(notice.args["version"], "0.9.0-beta.3");
    assert_eq!(notice.args["size"], "210 MB");
    assert_eq!(
        notice.args["notes"],
        "- Self-update - Plugins leave the package"
    );
    assert_eq!(notice.settings_key.as_deref(), Some("updates.enabled"));
    let action = action.expect("a button");
    assert_eq!(action.command, INSTALL_COMMAND);
    assert_eq!(action.label_key, "update.notice.install");
}

/// The acceptance: a Scoop installation shows the command and downloads nothing.
#[test]
fn a_package_manager_is_shown_its_command_and_no_button() {
    let (notice, action) = offer(&offered(true), InstallKind::Scoop);
    assert_eq!(notice.message_key, "update.notice.availableCommand");
    assert_eq!(notice.args["command"], "scoop update rbrowser");
    assert!(action.is_none());
}

#[test]
fn an_unknown_installation_or_a_release_without_this_builds_file_gets_the_download_page() {
    for (kind, artifact) in [
        (InstallKind::Unknown, true),
        (InstallKind::Msi, true),
        (InstallKind::Portable, false),
    ] {
        let (notice, action) = offer(&offered(artifact), kind);
        assert_eq!(notice.message_key, "update.notice.availableDownload");
        assert_eq!(
            notice.args["url"],
            "https://github.com/degoya/rBrowser/releases/tag/v0.9.0-beta.3"
        );
        let action = action.expect("a button");
        assert_eq!(action.command, OPEN_LINK_COMMAND);
        assert_eq!(action.args["url"], notice.args["url"].as_str());
    }
}

#[test]
fn long_notes_are_cut_and_sizes_round_up() {
    let long = "x".repeat(NOTES_CHARS + 50);
    let short = short_notes(&long);
    assert_eq!(short.chars().count(), NOTES_CHARS + 3);
    assert!(short.ends_with("..."));
    assert_eq!(megabytes(1), "1 MB");
    assert_eq!(megabytes(2_000_000), "2 MB");
}

#[test]
fn every_failure_code_has_a_sentence_and_names_its_code() {
    let codes = rb_update::UPDATE_ERROR_CODES
        .iter()
        .chain(rb_update::install::INSTALL_CODES);
    for code in codes {
        let (notice, action) = failure(code);
        assert!(notice.message_key.starts_with("update.error."), "{code}");
        assert_eq!(notice.args["code"], *code);
        assert_eq!(notice.level, NotificationLevel::Error);
        assert!(action.is_none());
    }
    assert_eq!(
        failure("update.bad_signature").0.message_key,
        "update.error.refused"
    );
    assert_eq!(
        failure("update.digest_mismatch").0.message_key,
        "update.error.download"
    );
}

#[test]
fn an_mcp_request_waits_for_the_persons_click() {
    let (notice, action) = mcp_asks(&offered(true));
    assert_eq!(notice.message_key, "update.notice.mcpAsks");
    assert_eq!(action.expect("a button").command, INSTALL_COMMAND);
}

/// RB-090-14 review: with other windows open the install waits, says why, and offers to try
/// again; it is no failure.
#[test]
fn other_windows_hold_the_install_with_a_warning_and_a_retry() {
    let (notice, action) = other_windows(2, "0.9.0-beta.3");
    assert_eq!(notice.message_key, "update.notice.otherWindows");
    assert_eq!(notice.level, NotificationLevel::Warning);
    assert_eq!(notice.args["count"], "2");
    assert_eq!(notice.args["version"], "0.9.0-beta.3");
    assert_eq!(action.expect("a button").command, INSTALL_COMMAND);
}
