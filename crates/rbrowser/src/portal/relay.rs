//! What the devices hear (RB-080-05). A device has no DevTools protocol, so the portal agent
//! replays in JavaScript (`isTrusted` false): navigation by `location`, scroll by anchor and
//! offset, a click by `element.click()`, input and form changes by value plus `input`/`change`,
//! focus by `focus()`. What script cannot replay is not sent and counts as skipped: hover (no
//! real `:hover`), pointer and wheel (scroll arrives as scroll), keys (text arrives as input),
//! the context menu, and a submit (the device follows the navigation it causes). A password
//! never leaves the machine: the portal speaks plain HTTP in the LAN. A device answers with
//! [`Report`]s only: its page is up, a replay ended, or an origin its page failed to load from
//! (RB-090-72, the origin alone, checked here - a device is untrusted).

use rb_sync::{SyncEvent, SyncKind, SyncSwitches, is_sensitive};
use rb_util::url::origin_of;
use serde::{Deserialize, Serialize};

use super::origins::device_path;

/// The longest message a device may send.
pub const MAX_REPORT: usize = 4096;
/// The longest origin a device may name.
pub const MAX_ORIGIN: usize = 255;

/// A message to a device.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ToDevice {
    /// Go to `path` on the portal (the active view's path, query and hash).
    Navigate {
        /// `/a?b#c`.
        path: String,
    },
    /// Replay `event`.
    Event {
        /// The leader's event (boxed: far larger than a path).
        event: Box<SyncEvent>,
    },
}

/// Why an event was not sent to the devices.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Skip {
    /// Sync, or this kind of it, is off.
    Off,
    /// A password field's value or key.
    Sensitive,
    /// Script cannot replay it.
    NotReplayable,
}

/// The message for `event` of the leader, or why there is none.
///
/// # Errors
///
/// [`Skip`].
pub fn for_device(event: &SyncEvent, switches: &SyncSwitches) -> Result<ToDevice, Skip> {
    if !switches.enabled || !switches.kind(SyncKind::of(event)) {
        return Err(Skip::Off);
    }
    if is_sensitive(event) {
        return Err(Skip::Sensitive);
    }
    match event {
        SyncEvent::Click(_)
        | SyncEvent::Dblclick(_)
        | SyncEvent::Scroll(_)
        | SyncEvent::Input(_)
        | SyncEvent::Change(_)
        | SyncEvent::Focus(_) => Ok(ToDevice::Event {
            event: Box::new(event.clone()),
        }),
        SyncEvent::Pointer(_)
        | SyncEvent::Contextmenu(_)
        | SyncEvent::Hover(_)
        | SyncEvent::Wheel(_)
        | SyncEvent::Keydown(_)
        | SyncEvent::Keyup(_)
        | SyncEvent::Submit(_) => Err(Skip::NotReplayable),
    }
}

/// The message that has the devices follow the active view to `url`.
#[must_use]
pub fn navigate(url: &str) -> Option<ToDevice> {
    device_path(url).map(|path| ToDevice::Navigate { path })
}

/// How a device's replay ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplayStatus {
    /// Replayed.
    Done,
    /// The element is not on the device's page.
    NotFound,
    /// The device's agent cannot replay it.
    Unsupported,
    /// The replay threw.
    Error,
}

/// A message of a device. Only these: a device can drive nothing in rBrowser.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Report {
    /// The agent of a new page is up.
    Hello,
    /// A replay ended.
    Replayed {
        /// How.
        status: ReplayStatus,
    },
    /// The page failed to load a resource from `origin` (RB-090-72): an image, a style sheet, a
    /// script, a font, a `fetch`. Only a bare origin ([`is_bare_origin`]).
    Missed {
        /// `http://localhost:5173`.
        origin: String,
        /// How the page asked for it, when the device says.
        #[serde(default)]
        via: Option<Via>,
    },
}

/// How a device's page asked for a resource that failed: the element, or the resource timing's
/// initiator type; any other word is [`Via::Other`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Via {
    /// An image.
    Img,
    /// A script.
    Script,
    /// A `<link>`: a style sheet, a preload, an icon.
    Link,
    /// A style sheet's `url()` or `@import`.
    Css,
    /// A `<source>` of a picture or a medium.
    Source,
    /// A video.
    Video,
    /// An audio.
    Audio,
    /// `fetch`.
    Fetch,
    /// `XMLHttpRequest`.
    Xmlhttprequest,
    /// Anything else.
    #[serde(other)]
    Other,
}

impl Via {
    /// The word, as the device sent it.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Img => "img",
            Self::Script => "script",
            Self::Link => "link",
            Self::Css => "css",
            Self::Source => "source",
            Self::Video => "video",
            Self::Audio => "audio",
            Self::Fetch => "fetch",
            Self::Xmlhttprequest => "xmlhttprequest",
            Self::Other => "other",
        }
    }
}

/// A device's message, when it is one of [`Report`] and not too long, and a missed origin is a
/// bare origin.
#[must_use]
pub fn parse_report(text: &str) -> Option<Report> {
    if text.len() > MAX_REPORT {
        return None;
    }
    let value: serde_json::Value = serde_json::from_str(text).ok()?;
    let report: Report = serde_json::from_value(value).ok()?;
    match &report {
        Report::Missed { origin, .. } if !is_bare_origin(origin) => None,
        _ => Some(report),
    }
}

/// Whether `origin` is a bare `http(s)` origin as a browser's `URL.origin` writes it: lower
/// case, no user info, path, query or fragment, no default port, a port that is a port, at most
/// [`MAX_ORIGIN`] bytes.
#[must_use]
pub fn is_bare_origin(origin: &str) -> bool {
    if origin.len() > MAX_ORIGIN
        || !origin
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b".-:/[]".contains(&byte))
        || origin_of(origin).as_deref() != Some(origin)
    {
        return false;
    }
    let authority = origin.split_once("://").map_or("", |(_, rest)| rest);
    let port = match authority.rfind(']') {
        Some(end) => authority[end + 1..].strip_prefix(':'),
        None => authority.split_once(':').map(|(_, port)| port),
    };
    port.is_none_or(|port| port.parse::<u16>().is_ok())
}

/// A device's replay counts, for the device list.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Counts {
    /// Replayed on the device.
    pub replayed: u64,
    /// Sent, but the device could not replay it.
    pub failed: u64,
    /// Not sent: script cannot replay it, or it held a password.
    pub skipped: u64,
}

impl Counts {
    /// Counts a device's report.
    pub fn note(&mut self, report: &Report) {
        match report {
            Report::Replayed {
                status: ReplayStatus::Done,
            } => self.replayed += 1,
            Report::Replayed { .. } => self.failed += 1,
            Report::Hello | Report::Missed { .. } => {}
        }
    }

    /// Counts an event that was not sent.
    pub fn skipped(&mut self, skip: Skip) {
        if skip != Skip::Off {
            self.skipped += 1;
        }
    }
}

#[cfg(test)]
#[path = "relay_tests.rs"]
mod tests;
