//! The Browsersync guard (RB-020-06): Browsersync's ghost mode replays clicks, scrolls and forms
//! between the browsers it serves, so rBrowser's own sync on top would replay every input twice.
//! When a view's agent reports Browsersync's client and `sync.detectBrowsersync` is on, sync is
//! switched off and the shell says why, with "sync anyway". Switching sync on again - from that
//! notice, the toolbar, `Leader S S` or MCP - counts as "sync anyway" for that site, for the rest
//! of the session.

use std::collections::BTreeSet;

/// What the window does about a detection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GuardAction {
    /// Switch sync off and say why.
    Pause,
    /// The person switched sync on again: the site is allowed, the notice goes.
    Allow,
    /// Nothing.
    Nothing,
}

/// Which sites paused sync and which the person allowed.
#[derive(Debug, Default)]
pub struct BrowsersyncGuard {
    allowed: BTreeSet<String>,
    paused: Option<String>,
}

impl BrowsersyncGuard {
    /// Browsersync is in a page of `origin`; `detect` is `sync.detectBrowsersync`, `sync_on`
    /// the global sync switch now.
    pub fn detected(&mut self, origin: &str, detect: bool, sync_on: bool) -> GuardAction {
        if !detect || !sync_on || self.allowed.contains(origin) {
            return GuardAction::Nothing;
        }
        if self.paused.as_deref() == Some(origin) {
            self.paused = None;
            self.allowed.insert(origin.to_owned());
            return GuardAction::Allow;
        }
        self.paused = Some(origin.to_owned());
        GuardAction::Pause
    }

    /// The site that paused sync, while its notice stands.
    pub fn paused(&self) -> Option<&str> {
        self.paused.as_deref()
    }

    /// Forgets the pause and the allowed sites (`sync.detectBrowsersync` changed).
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
#[path = "browsersync_tests.rs"]
mod tests;
