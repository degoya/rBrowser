//! The portal as its panel shows it (RB-080-05): whether it may run and runs, where devices
//! reach it, the paired devices and - in the answer to `getPortal` only - the open pairing with
//! its QR code. The pairing URL holds a one-time token: its `Debug` output is masked. Beside it
//! the presentation over the internet (RB-096-05): its phase, viewers and open link.

use std::fmt;

use serde::{Deserialize, Serialize};

/// The portal's state.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortalState {
    /// `portal.enabled`: the portal may run.
    pub enabled: bool,
    /// It listens.
    pub running: bool,
    /// `http://ip:port` while it listens.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
    /// The paired devices, oldest first.
    pub devices: Vec<PortalDevice>,
    /// The open pairing; absent when none is open (or in a pushed event).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pairing: Option<PortalPairing>,
    /// The presentation over the internet (RB-096-05).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub presentation: Option<PortalPresentation>,
}

/// Where a presentation stands.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PresentationPhase {
    /// None runs.
    #[default]
    Off,
    /// cloudflared is being downloaded for a start.
    Downloading,
    /// The tunnel opens.
    Starting,
    /// Viewers can join.
    Live,
}

/// The presentation over the internet as the Portal panel and the activity bar show it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortalPresentation {
    /// `portal.presentation.enabled`: it may run.
    pub enabled: bool,
    /// `portal.presentation.consented`: the person agreed to the traffic through Cloudflare.
    pub consented: bool,
    /// Where it stands.
    pub phase: PresentationPhase,
    /// `https://<name>.trycloudflare.com` once the tunnel named it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
    /// The viewers joined.
    pub viewers: u32,
    /// `portal.presentation.maxViewers` in effect.
    pub max_viewers: u32,
    /// When it started, ms since the epoch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at_ms: Option<u64>,
    /// The open link with its QR code (`getPortal` only; it holds the token).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link: Option<PortalPairing>,
    /// What a start would download first: cloudflared is missing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub download: Option<PresentationDownload>,
}

/// The cloudflared a start would download, for the consent.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PresentationDownload {
    /// The pinned version.
    pub version: String,
    /// The size in megabytes, one decimal.
    pub size_mb: String,
    /// The SPDX licence.
    pub license: String,
    /// The host it comes from.
    pub from: String,
}

/// A paired device.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortalDevice {
    /// The id `portal.disconnect` takes.
    pub id: u32,
    /// The user agent, shortened; empty when it sent none.
    pub name: String,
    /// The address it paired from.
    pub ip: String,
    /// When it paired, ms since the epoch.
    pub paired_at_ms: u64,
    /// Its last request, ms since the epoch.
    pub last_seen_ms: u64,
    /// It came back with its cookie at least once; `false` is "pairing not completed".
    pub contacted: bool,
    /// Its agent is connected now.
    pub connected: bool,
    /// Events replayed on it.
    pub replayed: u64,
    /// Events it could not replay.
    pub failed: u64,
    /// Events not sent: script cannot replay them, or they held a password.
    pub skipped: u64,
}

/// The open pairing: the URL in the QR code and the code's modules.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortalPairing {
    /// `http://ip:port/__rbrowser/pair?t=<token>`.
    pub url: String,
    /// Modules per side.
    pub size: u32,
    /// `size` rows of `0` (light) and `1` (dark).
    pub rows: Vec<String>,
    /// When the token runs out, ms since the epoch.
    pub expires_at_ms: u64,
}

impl fmt::Debug for PortalPairing {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PortalPairing")
            .field("size", &self.size)
            .field("expires_at_ms", &self.expires_at_ms)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
#[path = "portal_tests.rs"]
mod tests;
