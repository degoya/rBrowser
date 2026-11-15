//! What the devices hear (RB-080-05). A device has no DevTools protocol, so the portal agent
//! replays in JavaScript (`isTrusted` false): navigation by `location`, scroll by anchor and
//! offset, a click by `element.click()`, input and form changes by value plus `input`/`change`,
//! focus by `focus()`. What script cannot replay is not sent and counts as skipped: hover (no
//! real `:hover`), pointer and wheel (scroll arrives as scroll), keys (text arrives as input),
//! the context menu, and a submit (the device follows the navigation it causes). A password
//! never leaves the machine: the portal speaks plain HTTP in the LAN.

use rb_sync::{SyncEvent, SyncKind, SyncSwitches, is_sensitive};
use serde::{Deserialize, Serialize};

use super::origins::device_path;

/// The longest message a device may send.
pub const MAX_REPORT: usize = 4096;

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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Report {
    /// The agent of a new page is up.
    Hello,
    /// A replay ended.
    Replayed {
        /// How.
        status: ReplayStatus,
    },
}

/// A device's message, when it is one of [`Report`] and not too long.
#[must_use]
pub fn parse_report(text: &str) -> Option<Report> {
    if text.len() > MAX_REPORT {
        return None;
    }
    let value: serde_json::Value = serde_json::from_str(text).ok()?;
    serde_json::from_value(value).ok()
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
            Report::Hello => {}
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
