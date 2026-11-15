//! The switch in front of everything that reaches the network: self-update runs only in a build
//! with the Cargo feature `self-update` (of the app, off in phase 1) **and** with the setting
//! `updates.enabled` on (off by default, protected from MCP). While either is off, [`check`]
//! answers `update.disabled` before it builds a trust store, reads a file or asks the fetcher
//! for anything - `gate_tests.rs` proves that with a fetcher that records every request.
//!
//! Then the order: the compiled-in root (`update.not_configured` while it is the placeholder,
//! still without a request), the manifest of the channel, its verification against the stored
//! floor, and the raised floor stored before the manifest is handed on.

use std::path::Path;

use chrono::{DateTime, Utc};

use crate::error::UpdateError;
use crate::fetch::{BodyTooLarge, Fetcher, Url};
use crate::floors::Floors;
use crate::manifest::{self, Channel, MAX_MANIFEST_BYTES, UpdateManifest};
use crate::sign::TrustStore;

/// Whether self-update may act at all.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Gate {
    /// The app was built with the Cargo feature `self-update`.
    pub feature_built: bool,
    /// The setting `updates.enabled`.
    pub enabled: bool,
}

impl Gate {
    /// Open only when both are on.
    #[must_use]
    pub fn is_open(self) -> bool {
        self.feature_built && self.enabled
    }
}

/// Fetches and verifies the manifest of `channel` below `base` (the release download prefix,
/// ending in `/`), with the floors of `data`; see the module documentation.
///
/// # Errors
///
/// `update.disabled` while the gate is shut, otherwise as [`check_with`].
pub fn check(
    gate: Gate,
    fetcher: &dyn Fetcher,
    base: &str,
    channel: Channel,
    data: &Path,
    now: DateTime<Utc>,
) -> Result<UpdateManifest, UpdateError> {
    if !gate.is_open() {
        return Err(UpdateError::Disabled);
    }
    let trust = manifest::release_trust(now)?;
    check_with(&trust, fetcher, base, channel, data, now)
}

/// [`check`] behind an open gate, against `trust`.
///
/// # Errors
///
/// `update.invalid` for a base that is not https, `update.not_published` for a `404`,
/// `update.too_large`, `update.fetch_failed`, every refusal of [`manifest::verify_with`], and
/// `update.failed` when the floor cannot be stored.
pub fn check_with(
    trust: &TrustStore,
    fetcher: &dyn Fetcher,
    base: &str,
    channel: Channel,
    data: &Path,
    now: DateTime<Utc>,
) -> Result<UpdateManifest, UpdateError> {
    if !manifest::is_plain_https(base) || !base.ends_with('/') {
        return Err(UpdateError::Invalid(format!(
            "{base} is no https:// download prefix"
        )));
    }
    let url = Url::parse(base)
        .and_then(|base| base.join(channel.file_name()))
        .map_err(|error| UpdateError::Invalid(error.to_string()))?;
    let limit = u64::try_from(MAX_MANIFEST_BYTES).unwrap_or(u64::MAX);
    let bytes = match fetcher.fetch(&url, limit) {
        Ok(Some(bytes)) => bytes,
        Ok(None) => return Err(UpdateError::NotPublished),
        Err(error) if error.downcast_ref::<BodyTooLarge>().is_some() => {
            return Err(UpdateError::TooLarge);
        }
        Err(error) => return Err(UpdateError::Fetch(format!("{error:#}"))),
    };
    let mut floors = Floors::load(data);
    let manifest = manifest::verify_with(&bytes, trust, channel, floors.get(channel), now)?;
    floors.raise(channel, manifest.sequence);
    floors.store(data).map_err(UpdateError::Other)?;
    Ok(manifest)
}

#[cfg(test)]
#[path = "gate_tests.rs"]
mod tests;
