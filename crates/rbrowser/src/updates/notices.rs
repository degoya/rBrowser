//! The status bar's sentences about an update (RB-090-14): the offer with its button (version,
//! size, the first of the notes), what happens next, and every failure by a key of its own.
//! Built here as values, shown by `flow.rs`.

use std::collections::BTreeMap;

use rb_ipc::{Notification, NotificationLevel, StatusAction};
use rb_update::{InstallKind, Offer, Sources, UpdateWay};
use serde_json::{Value, json};

use super::INSTALL_COMMAND;
use crate::open_link::OPEN_LINK_COMMAND;

/// The settings the notices link to: General > Updates.
const SETTINGS_KEY: &str = "updates.enabled";
/// The most of the notes a notice shows; the rest is in `update_status` and the release.
pub(super) const NOTES_CHARS: usize = 280;

/// A notice and the button it may carry.
pub(super) type Notice = (Notification, Option<StatusAction>);

fn notice(level: NotificationLevel, key: &str, args: &[(&str, String)]) -> Notification {
    Notification {
        level,
        message_key: key.to_owned(),
        args: args
            .iter()
            .map(|(name, value)| ((*name).to_owned(), value.clone()))
            .collect(),
        settings_key: Some(SETTINGS_KEY.to_owned()),
    }
}

fn button(command: &str, label_key: &str, args: Value) -> StatusAction {
    StatusAction {
        command: command.to_owned(),
        label_key: label_key.to_owned(),
        label_args: BTreeMap::new(),
        args,
    }
}

/// `size` bytes in whole megabytes, rounded up: `210 MB`.
pub(super) fn megabytes(size: u64) -> String {
    format!("{} MB", size.div_ceil(1_000_000))
}

/// The notes on one line, cut at [`NOTES_CHARS`] characters.
pub(super) fn short_notes(notes: &str) -> String {
    let line = notes
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    if line.chars().count() <= NOTES_CHARS {
        return line;
    }
    let cut: String = line.chars().take(NOTES_CHARS).collect();
    format!("{}...", cut.trim_end())
}

/// The offer as this installation takes it: "Install and restart" for the portable archive, the
/// package manager's command, or the release page.
pub(super) fn offer(offer: &Offer, kind: InstallKind) -> Notice {
    let version = ("version", offer.version.clone());
    let size = ("size", offer.size().map(megabytes).unwrap_or_default());
    let notes = ("notes", short_notes(&offer.notes));
    match (kind.update_way(), offer.artifact.is_some()) {
        (UpdateWay::SelfUpdate, true) => (
            notice(
                NotificationLevel::Info,
                "update.notice.available",
                &[version, size, notes],
            ),
            Some(button(
                INSTALL_COMMAND,
                "update.notice.install",
                Value::Null,
            )),
        ),
        (UpdateWay::Command(command), _) => (
            notice(
                NotificationLevel::Info,
                "update.notice.availableCommand",
                &[version, ("command", command.to_owned()), notes],
            ),
            None,
        ),
        _ => download(offer),
    }
}

/// The release page, for an installation that is updated by hand.
pub(super) fn download(offer: &Offer) -> Notice {
    let url = Sources::release_page(&offer.version);
    (
        notice(
            NotificationLevel::Info,
            "update.notice.availableDownload",
            &[("version", offer.version.clone()), ("url", url.clone())],
        ),
        Some(button(
            OPEN_LINK_COMMAND,
            "update.notice.download",
            json!({ "url": url }),
        )),
    )
}

/// An MCP client asked to install: the person decides with the button.
pub(super) fn mcp_asks(offer: &Offer) -> Notice {
    (
        notice(
            NotificationLevel::Warning,
            "update.notice.mcpAsks",
            &[
                ("version", offer.version.clone()),
                ("size", offer.size().map(megabytes).unwrap_or_default()),
            ],
        ),
        Some(button(
            INSTALL_COMMAND,
            "update.notice.install",
            Value::Null,
        )),
    )
}

/// "Install and restart" while other projects' windows run: the updater would wait for them in
/// vain. Nothing is marked failed; the button tries again once they are closed.
pub(super) fn other_windows(count: usize, version: &str) -> Notice {
    (
        notice(
            NotificationLevel::Warning,
            "update.notice.otherWindows",
            &[
                ("count", count.to_string()),
                ("version", version.to_owned()),
            ],
        ),
        Some(button(
            INSTALL_COMMAND,
            "update.notice.install",
            Value::Null,
        )),
    )
}

/// A sentence without a button.
pub(super) fn plain(level: NotificationLevel, key: &str, args: &[(&str, String)]) -> Notice {
    (notice(level, key, args), None)
}

/// A failure by its code (`update.*` of `rb_update`): the key that says it in words, the code
/// beside it for the log and a report.
pub(super) fn failure(code: &str) -> Notice {
    let key = match code {
        "update.fetch_failed" | "update.not_published" => "update.error.unreachable",
        "update.too_large"
        | "update.malformed"
        | "update.untrusted"
        | "update.bad_signature"
        | "update.revoked"
        | "update.schema_version_unsupported"
        | "update.wrong_channel"
        | "update.stale"
        | "update.invalid" => "update.error.refused",
        "update.download_failed" | "update.size_mismatch" | "update.digest_mismatch" => {
            "update.error.download"
        }
        "update.no_space" | "update.not_enough_space" => "update.error.noSpace",
        "update.install_dir_not_writable" => "update.error.notWritable",
        "update.updater_missing" => "update.error.updaterMissing",
        "update.busy" => "update.error.busy",
        "update.disabled" => "update.error.disabled",
        _ => "update.error.failed",
    };
    plain(NotificationLevel::Error, key, &[("code", code.to_owned())])
}

#[cfg(test)]
#[path = "notices_tests.rs"]
mod tests;
