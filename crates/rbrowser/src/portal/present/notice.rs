//! The status bar's words about a presentation (RB-096-05): its start, its end and every abort,
//! each linked to `portal.presentation.enabled`. None carries the link, its token or the public
//! address: the status history is readable over MCP (`status_messages`).

use rb_ipc::{CoreToShell, Notification, NotificationLevel};

/// The setting every notice links to.
pub const SETTINGS_KEY: &str = "portal.presentation.enabled";
/// The presentation is off in the settings.
pub const OFF: &str = "portal.present.notice.off";
/// The first start waits for the person's consent in the Portal panel.
pub const CONSENT: &str = "portal.present.notice.consent";
/// cloudflared is missing; `{version}`, `{size}`, `{license}`, `{from}`.
pub const NEEDS_DOWNLOAD: &str = "portal.present.notice.needs_download";
/// cloudflared is being downloaded; the same placeholders.
pub const DOWNLOADING: &str = "portal.present.notice.downloading";
/// The tunnel opens.
pub const STARTING: &str = "portal.present.notice.starting";
/// The presentation is live.
pub const STARTED: &str = "portal.present.notice.started";
/// A person or MCP stopped it.
pub const STOPPED: &str = "portal.present.notice.stopped";
/// cloudflared ended by itself.
pub const ABORTED: &str = "portal.present.notice.aborted";
/// The tunnel did not connect in time.
pub const TIMEOUT: &str = "portal.present.notice.timeout";
/// cloudflared named an address that is no quick tunnel's.
pub const FAILED_ADDRESS: &str = "portal.present.notice.failed_address";
/// It did not start; `{error}`.
pub const FAILED: &str = "portal.present.notice.failed";

/// A notification of `key` with `args`, linked to the presentation's setting.
#[must_use]
pub fn notice(level: NotificationLevel, key: &str, args: &[(&str, String)]) -> CoreToShell {
    CoreToShell::Notification(Notification {
        level,
        message_key: key.to_owned(),
        args: args
            .iter()
            .map(|(name, value)| ((*name).to_owned(), value.clone()))
            .collect(),
        settings_key: Some(SETTINGS_KEY.to_owned()),
    })
}
