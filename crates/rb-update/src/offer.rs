//! Which verified release, if any, this installation is offered (RB-090-14).
//!
//! * **No downgrade.** Only a version of higher SemVer precedence than the running one, and one
//!   whose `min_version` the running one meets ([`UpdateManifest::updates_from`]).
//! * **No beta on the stable channel.** The beta channel sees stable releases too, so
//!   `0.9.0-beta.3` is offered `0.9.0`.
//! * **The running build's file.** The artifact for this platform, architecture and kind; an
//!   offer without one is shown, and only downloaded by hand.
//!
//! Adapted from rCommander's `crates/rc-update/src/offer.rs`: no database schema flag.

use serde::Serialize;
use time::OffsetDateTime;

use crate::manifest::{Artifact, Channel, UpdateManifest, kind};

/// What the window shows about an available update.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Offer {
    pub version: String,
    pub channel: Channel,
    #[serde(serialize_with = "crate::utc::serialize")]
    pub released_at: OffsetDateTime,
    /// Plain text, the version's section of the changelog.
    pub notes: String,
    /// The file for this platform and architecture, when the release has one.
    pub artifact: Option<Artifact>,
}

impl Offer {
    /// The download's size in bytes, when there is a file for this build.
    #[must_use]
    pub fn size(&self) -> Option<u64> {
        self.artifact.as_ref().map(|artifact| artifact.size)
    }
}

/// The artifact an installation takes: its platform, architecture and kind.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Target {
    pub platform: &'static str,
    pub arch: &'static str,
    pub kind: &'static str,
}

impl Target {
    /// The running build's: the portable ZIP of its platform and architecture.
    #[must_use]
    pub fn current() -> Self {
        Self {
            platform: match std::env::consts::OS {
                "macos" => "macos",
                "windows" => "windows",
                _ => "linux",
            },
            arch: std::env::consts::ARCH,
            kind: kind::ZIP,
        }
    }
}

/// The channel a build reads: the person's choice when they made one (`Some`); otherwise
/// (`updates.channel = auto`, the default) beta for a pre-release build (`0.9.0-beta.2`), which
/// would else never see the next beta of its own line, and stable for a release build.
#[must_use]
pub fn effective_channel(running: &str, setting: Option<Channel>) -> Channel {
    if let Some(chosen) = setting {
        return chosen;
    }
    match crate::manifest::parse_version(running) {
        Some(version) if !version.pre.is_empty() => Channel::Beta,
        _ => Channel::Stable,
    }
}

/// The newest release among the verified `manifests` that `channel` may offer over `current`,
/// with the artifact for `target`; `None` when this installation is up to date.
#[must_use]
pub fn newest_offer(
    current: &str,
    channel: Channel,
    manifests: &[UpdateManifest],
    target: &Target,
) -> Option<Offer> {
    manifests
        .iter()
        .filter_map(|manifest| Some((manifest, manifest.semver().ok()?)))
        .filter(|(manifest, version)| match channel {
            Channel::Stable => manifest.channel == Channel::Stable && version.pre.is_empty(),
            Channel::Beta => true,
        })
        .filter(|(manifest, _)| manifest.updates_from(current))
        .max_by(|(_, left), (_, right)| left.cmp_precedence(right))
        .map(|(manifest, _)| Offer {
            version: manifest.version.clone(),
            channel: manifest.channel,
            released_at: manifest.released_at,
            notes: manifest.notes.clone(),
            artifact: manifest
                .artifact(target.platform, target.arch, target.kind)
                .cloned(),
        })
}

#[cfg(test)]
#[path = "offer_tests.rs"]
mod tests;
