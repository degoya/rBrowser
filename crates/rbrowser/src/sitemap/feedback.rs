//! What the person sees when a capture starts (RB-090-83; the owner: "when I start a function I
//! should be shown that something happens"): after every start - the command, MCP, the command
//! line - the notice "Sitemap capture started: N pages x M breakpoints", while the sitemap bar
//! opens by itself on a run it has not shown yet, whatever `panels.sitemap` did
//! (`ui/src/sitemap/SitemapBar.vue`). A second start from the person while one runs opens the bar
//! on the running capture and says so, instead of only failing with `sitemap.error.busy`.

use rb_ipc::{CoreToShell, NotificationLevel};

use super::app::notice;
use super::defs::PANEL;
use super::tool::CaptureStarted;
use crate::control::Core;

/// The notice of a start; placeholders `{pages}`, `{breakpoints}`, `{pending}`.
pub const STARTED_KEY: &str = "sitemap.notice.started";
/// The notice of a start while a capture runs.
pub const BUSY_KEY: &str = "sitemap.notice.busy";

/// The notice of `started`.
#[must_use]
pub fn started_notice(started: &CaptureStarted) -> CoreToShell {
    notice(
        NotificationLevel::Info,
        STARTED_KEY,
        &[
            ("pages", started.pages.to_string()),
            ("breakpoints", started.breakpoints.to_string()),
            ("pending", started.pending.to_string()),
        ],
    )
}

/// Tells the person that `started` began.
pub fn started(core: &Core, started: &CaptureStarted) {
    core.notify(started_notice(started));
}

/// What a start while a capture runs sends: the bar opened on it, then the notice.
#[must_use]
pub fn busy_messages() -> [CoreToShell; 2] {
    [
        CoreToShell::UiCommand {
            id: PANEL.to_owned(),
            args: None,
        },
        notice(NotificationLevel::Info, BUSY_KEY, &[]),
    ]
}

/// Shows the person the capture that runs already.
pub fn busy(core: &Core) {
    let [open, said] = busy_messages();
    core.send(&open);
    core.notify(said);
}

#[cfg(test)]
#[path = "feedback_tests.rs"]
mod tests;
