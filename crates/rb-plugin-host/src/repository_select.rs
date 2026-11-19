//! Choosing what the repositories offer for a first-party plugin (RB-091-04, adapted from
//! rDownloader's `repository_select.rs`, RD-1260-02): the repository's version of a bundled
//! package stands in for it when it is newer than the bundle's, of the same plugin, signed
//! by the same key and asking for nothing more - so an install of a chosen plugin needs no
//! review beyond the one confirmation of the whole choice. Anything else leaves the install to
//! the bundle, and a newer version that asks for more waits in the catalogue as an update to
//! review. A host that failed is not asked again for [`UNREACHABLE_FOR`]: without a network every
//! plugin the wizard installs would wait out its own timeout otherwise.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::index::Offer;
use crate::manifest::PluginManifest;
use crate::sign::{decode_public_key, key_fingerprint};

/// How long a failed fetch leaves installs from that host to the bundle.
pub const UNREACHABLE_FOR: Duration = Duration::from_secs(300);

/// What is shipped or installed of a plugin, as the rule compares an offer with it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shipped {
    pub id: String,
    pub version: semver::Version,
    /// The fingerprint of the key its manifest names.
    pub fingerprint: String,
    /// The capability ids it is granted.
    pub capabilities: BTreeSet<String>,
}

impl Shipped {
    /// `manifest` as the rule reads it; `None` when its public key does not decode.
    #[must_use]
    pub fn of(manifest: &PluginManifest) -> Option<Self> {
        let key = decode_public_key(&manifest.public_key).ok()?;
        Some(Self {
            id: manifest.id.clone(),
            version: manifest.version.clone(),
            fingerprint: key_fingerprint(&key),
            capabilities: manifest
                .grants
                .granted
                .iter()
                .map(|capability| capability.id().to_owned())
                .collect(),
        })
    }
}

impl Shipped {
    /// `self` at `version` when that is newer: a version installed in this session, which loads
    /// at the next start, is the base of the next update (review 6). A version that does not
    /// parse changes nothing.
    #[must_use]
    pub fn at_least(mut self, version: &str) -> Self {
        if let Ok(installed) = semver::Version::parse(version)
            && installed > self.version
        {
            self.version = installed;
        }
        self
    }
}

/// The version of `offer` when it may stand in for `shipped`: compatible, the same plugin and
/// key, no capability `shipped` lacks.
fn stands_in(offer: &Offer, shipped: &Shipped) -> Option<semver::Version> {
    if !offer.compatibility.is_compatible()
        || offer.entry.id != shipped.id
        || offer.entry.publisher.fingerprint != shipped.fingerprint
        || offer
            .entry
            .capabilities
            .iter()
            .any(|capability| !shipped.capabilities.contains(capability))
    {
        return None;
    }
    semver::Version::parse(&offer.entry.version).ok()
}

fn best<'a>(
    offers: &'a [Offer],
    shipped: &Shipped,
    keep: impl Fn(&semver::Version) -> bool,
) -> Option<&'a Offer> {
    let mut found: Option<(&Offer, semver::Version)> = None;
    for offer in offers {
        let Some(version) = stands_in(offer, shipped).filter(|version| keep(version)) else {
            continue;
        };
        if found.as_ref().is_none_or(|(_, known)| version > *known) {
            found = Some((offer, version));
        }
    }
    found.map(|(offer, _)| offer)
}

/// The offer an install of the bundled `shipped` takes instead of the file beside the program:
/// the newest above the bundle's version - the bundle's own version installs from the bundle, as
/// the loader lets only a newer copy stand in (review 5). `None` leaves the install to the bundle.
#[must_use]
pub fn replacement<'a>(offers: &'a [Offer], shipped: &Shipped) -> Option<&'a Offer> {
    newer_than(offers, shipped)
}

/// The newest offer strictly above `shipped`, under the same rule: the repository's version of a
/// bundled package ([`replacement`]) and an update that may install without a review
/// (`plugins.catalog.autoUpdate`).
#[must_use]
pub fn newer_than<'a>(offers: &'a [Offer], shipped: &Shipped) -> Option<&'a Offer> {
    best(offers, shipped, |version| *version > shipped.version)
}

/// The hosts that failed lately, and until when they rest.
#[derive(Debug, Default)]
pub struct Reachability(Mutex<BTreeMap<String, Instant>>);

impl Reachability {
    /// Whether the host of `url` answered its last fetch, or failed it [`UNREACHABLE_FOR`] or
    /// longer before `now`.
    #[must_use]
    pub fn reachable(&self, url: &url::Url, now: Instant) -> bool {
        let Some(host) = url.host_str() else {
            return false;
        };
        rb_util::lock(&self.0)
            .get(host)
            .is_none_or(|until| now >= *until)
    }

    /// Records how the last fetch from the host of `url` went at `now`.
    pub fn note(&self, url: &url::Url, reached: bool, now: Instant) {
        let Some(host) = url.host_str() else {
            return;
        };
        let mut hosts = rb_util::lock(&self.0);
        if reached {
            hosts.remove(host);
        } else {
            hosts.insert(host.to_owned(), now + UNREACHABLE_FOR);
        }
    }
}

#[cfg(test)]
#[path = "repository_select_tests.rs"]
mod tests;
