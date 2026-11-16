//! The portal as its panel shows it (RB-080-05): whether it may run and runs, where devices
//! reach it, the paired devices and - in the answer to `getPortal` only - the open pairing with
//! its QR code. The pairing URL holds a one-time token: its `Debug` output is masked.

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
