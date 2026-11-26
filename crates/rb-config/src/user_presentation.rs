//! `portal.presentation` of the user settings (RB-096-05): the portal shown to a customer over
//! the internet through a Cloudflare Quick Tunnel. Off by default, and every key is protected
//! from MCP: each one widens what reaches the internet. Settings > Security & Privacy > Portal
//! and pairing.

use std::time::Duration;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The shortest life of a viewer link, in seconds.
pub const MIN_LINK_TTL_SEC: u32 = 60;
/// The longest life of a viewer link, in seconds.
pub const MAX_LINK_TTL_SEC: u32 = 3600;
/// The most viewers a presentation admits at once.
pub const MAX_VIEWERS: u32 = 10;

/// `portal.presentation`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct PresentationSettings {
    /// A presentation over the internet may run. Off, `portal.present.start` refuses; on, it
    /// still starts only when a person runs it (MCP cannot).
    pub enabled: bool,
    /// The person agreed that the presentation's traffic runs through Cloudflare (a Quick
    /// Tunnel, meant for tests and demonstrations). Set by the consent in the Portal panel.
    pub consented: bool,
    /// How long a viewer link stays valid, in seconds. Each link works once.
    #[schemars(range(min = 60, max = 3600))]
    pub link_ttl_sec: u32,
    /// How many viewers may watch at once.
    #[schemars(range(min = 1, max = 10))]
    pub max_viewers: u32,
    /// The paths a viewer may load (`/shop` allows `/shop` and everything below it). Empty: the
    /// shown environment's start page and everything below it.
    pub allow_paths: Vec<String>,
    /// Paths a viewer never loads, even below an allowed one: they answer 404 and never reach
    /// the dev server (`/admin` also covers `/admin/users` and `/administrator`). A path that
    /// begins with `.` or `__` is matched at every segment (`/.git` also covers
    /// `/sub/.git/config`, `/__open-in-editor` also `/app/__open-in-editor`).
    pub deny_paths: Vec<String>,
    /// A short flash on the viewer's page where the presenter clicks.
    pub pointer: bool,
}

impl Default for PresentationSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            consented: false,
            link_ttl_sec: 600,
            max_viewers: 3,
            allow_paths: Vec::new(),
            deny_paths: [
                "/admin",
                "/.env",
                "/.git",
                "/_debug",
                "/@fs/",
                // A viewer opens no file in the presenter's editor and reads no source: Vite's
                // and Vue CLI's editor launch, Create React App's, Next.js' and its stack frames.
                "/__open-in-editor",
                "/__open-stack-frame-in-editor",
                "/__nextjs_launch-editor",
                "/__nextjs_original-stack-frame",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect(),
            pointer: true,
        }
    }
}

impl PresentationSettings {
    /// A viewer link's life, held to its range.
    #[must_use]
    pub fn link_ttl(&self) -> Duration {
        let seconds = self.link_ttl_sec.clamp(MIN_LINK_TTL_SEC, MAX_LINK_TTL_SEC);
        Duration::from_secs(u64::from(seconds))
    }

    /// The viewer limit, held to its range.
    #[must_use]
    pub fn viewer_limit(&self) -> usize {
        usize::try_from(self.max_viewers.clamp(1, MAX_VIEWERS)).unwrap_or(1)
    }

    /// The keys a running presentation was started with: a change of any of them ends it; the
    /// consent is none of them.
    #[must_use]
    pub fn session(&self) -> Self {
        Self {
            consented: false,
            ..self.clone()
        }
    }
}

#[cfg(test)]
#[path = "user_presentation_tests.rs"]
mod tests;
