//! `portal` of the user settings (RB-080-05): real devices in the LAN join the sync through a
//! listener of their own. Off by default, and every key is protected from MCP: each one widens
//! what reaches the network. Settings > Security & Privacy > Portal and pairing.

use std::time::Duration;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The shortest life of a pairing token, in seconds.
pub const MIN_PAIRING_TTL_SEC: u32 = 30;
/// The longest life of a pairing token, in seconds.
pub const MAX_PAIRING_TTL_SEC: u32 = 3600;
/// The most devices the portal admits at once.
pub const MAX_DEVICES: u32 = 20;

/// `portal`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalSettings {
    /// The portal may run. Off, `portal.toggle` refuses; on, the portal still listens only once a
    /// person starts it (MCP cannot).
    pub enabled: bool,
    /// The IPv4 address of the LAN interface the portal listens on; empty: the interface of the
    /// default route, and only when its address is private (RFC 1918, 169.254/16, 100.64/10) -
    /// a public or VPN address refuses to start. Never every interface (`0.0.0.0`).
    pub interface: String,
    /// A fixed port; a free one at every start when absent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    /// How long the pairing code in the QR code stays valid, in seconds.
    #[schemars(range(min = 30, max = 3600))]
    pub pairing_ttl_sec: u32,
    /// How many devices may be paired at once.
    #[schemars(range(min = 1, max = 20))]
    pub max_devices: u32,
    /// When a portal page names a local origin (`http://localhost:5173`) the shown environment
    /// does not list in `assetOrigins`, the status bar offers to add it; a person's click writes
    /// the project config. Off, the log names the origin only (RB-090-71).
    pub offer_asset_origins: bool,
    /// The portal shown over the internet through a Cloudflare Quick Tunnel (RB-096-05).
    pub presentation: crate::PresentationSettings,
}

impl Default for PortalSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            interface: String::new(),
            port: None,
            pairing_ttl_sec: 300,
            max_devices: 5,
            offer_asset_origins: true,
            presentation: crate::PresentationSettings::default(),
        }
    }
}

impl PortalSettings {
    /// The pairing token's life, held to its range.
    #[must_use]
    pub fn pairing_ttl(&self) -> Duration {
        let seconds = self
            .pairing_ttl_sec
            .clamp(MIN_PAIRING_TTL_SEC, MAX_PAIRING_TTL_SEC);
        Duration::from_secs(u64::from(seconds))
    }

    /// The keys that make the listener: a change of any of them stops a running portal; the
    /// offer of asset origins is read where it is used, the presentation has its own listener.
    #[must_use]
    pub fn listener(&self) -> Self {
        Self {
            offer_asset_origins: true,
            presentation: crate::PresentationSettings::default(),
            ..self.clone()
        }
    }

    /// The device limit, held to its range.
    #[must_use]
    pub fn device_limit(&self) -> usize {
        usize::try_from(self.max_devices.clamp(1, MAX_DEVICES)).unwrap_or(1)
    }
}

#[cfg(test)]
#[path = "user_portal_tests.rs"]
mod tests;
