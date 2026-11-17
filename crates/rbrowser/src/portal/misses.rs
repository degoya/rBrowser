//! What a device failed to load (RB-090-72). The body scan
//! ([`super::origin_map::log_unmapped_loopback`]) finds only URLs in a body the portal read; a
//! URL built by script, inside a JSON answer that streamed through unread, or in an answer the
//! device had cached reaches the device untouched, and its failure is the only evidence. The
//! portal agent names each origin its page failed to load from (`agent/src/portal/misses.ts`,
//! [`super::relay::Report::Missed`], a bare origin checked by the relay), and the origin takes
//! the scan's way: logged once per origin and start of the portal - the origin only - and a
//! loopback `http` origin the portal does not forward goes to the offer
//! ([`super::offer::offer`], which `portal.offerAssetOrigins` governs). Any other origin is
//! logged with the hint that the portal forwards only the machine's local `http` origins. A
//! device is untrusted: each socket takes [`MAX_SOCKET_MISSES`] origins at most, the log and the
//! offer keep their own bounds, and only a person's click adds an origin.

use super::offer::{is_loopback, offer};
use super::origin_map::{OriginMap, first_logged_by_device};
use super::relay::Via;

/// The most origins one device socket (one page) may name.
pub const MAX_SOCKET_MISSES: usize = 16;

/// What a device's miss tells.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Miss {
    /// A portal origin (the page's listener or an asset listener): the upstream's own failure.
    Portal,
    /// The page's own upstream origin, asked for directly: a URL that reached the device
    /// unrewritten (built by script from parts, or in a body the portal does not rewrite - a
    /// compressed one, a type that is not text).
    PageUnrewritten,
    /// An asset origin the portal forwards, asked for directly, the same way.
    AssetUnrewritten,
    /// A loopback `http` origin the portal does not forward: an `assetOrigins` entry is missing.
    Unmapped,
    /// Any other origin - a LAN host, a `.test` name, `https`: the device must reach it itself.
    Elsewhere,
}

/// What a miss of `origin` tells while `map` is the shown page's (none without an allowed page).
#[must_use]
pub fn classify(origin: &str, map: Option<&OriginMap>) -> Miss {
    match map {
        Some(map) if map.portals().any(|portal| portal == origin) => Miss::Portal,
        Some(map) if map.main_upstream() == origin => Miss::PageUnrewritten,
        Some(map) if map.upstreams().any(|upstream| upstream == origin) => Miss::AssetUnrewritten,
        _ if is_loopback(origin) => Miss::Unmapped,
        _ => Miss::Elsewhere,
    }
}

/// The origins one socket named: each once, at most [`MAX_SOCKET_MISSES`].
#[derive(Debug, Default)]
pub struct SocketMisses {
    taken: Vec<String>,
}

impl SocketMisses {
    /// Whether `origin` is new to this socket and the socket may still name one.
    pub fn admit(&mut self, origin: &str) -> bool {
        if self.taken.len() >= MAX_SOCKET_MISSES || self.taken.iter().any(|known| known == origin) {
            return false;
        }
        self.taken.push(origin.to_owned());
        true
    }

    /// How many origins the socket named.
    #[must_use]
    pub fn len(&self) -> usize {
        self.taken.len()
    }

    /// Whether the socket named none.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.taken.is_empty()
    }
}

/// A device's page failed to load from `origin` (asked for `via`, when the device says) while
/// `map` is the shown page's: logged once per origin and start, an unmapped loopback origin
/// offered with the page's origin. Called outside every lock.
pub fn device_missed(origin: &str, via: Option<Via>, map: Option<&OriginMap>) {
    let via = via.map_or("unknown", Via::as_str);
    match classify(origin, map) {
        Miss::Portal => {
            tracing::debug!(%origin, %via, "a portal device failed to load from a portal origin");
        }
        Miss::PageUnrewritten => {
            if first_logged_by_device(origin) {
                tracing::info!(
                    %origin,
                    %via,
                    "the page's own origin reached a portal device unrewritten - a URL built by \
                     script or in a body the portal does not rewrite; the device cannot reach it"
                );
            }
        }
        Miss::AssetUnrewritten => {
            if first_logged_by_device(origin) {
                tracing::info!(
                    %origin,
                    %via,
                    "an asset origin reached a portal device unrewritten - a URL built by script \
                     or in a body the portal does not rewrite; the device cannot reach it"
                );
            }
        }
        Miss::Unmapped => {
            if first_logged_by_device(origin) {
                tracing::info!(
                    %origin,
                    %via,
                    "a portal device failed to load from a loopback origin the portal does not \
                     forward; list it in the environment's assetOrigins"
                );
            }
            if let Some(map) = map {
                offer(map, origin);
            }
        }
        Miss::Elsewhere => {
            if first_logged_by_device(origin) {
                tracing::info!(
                    %origin,
                    %via,
                    "a portal device failed to load from an origin that is not this machine's \
                     loopback; the portal forwards only local http origins, the device must \
                     reach this one itself"
                );
            }
        }
    }
}

#[cfg(test)]
#[path = "misses_tests.rs"]
mod tests;
