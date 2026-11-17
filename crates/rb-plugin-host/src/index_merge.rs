//! Several repositories' verified indexes as one catalogue (RB-091-01).
//!
//! Adapted from rDownloader's `crates/rd-plugin-host/src/index_merge.rs` and
//! `repository_select.rs` (RD-140-01). **The same plugin id from two repositories stays two
//! offers**: each names its repository, the interface shows both origins, and an install names
//! the repository, id and version it takes - nothing is picked or replaced silently. An update is
//! offered from the repository a plugin was installed from only, so a second repository listing a
//! higher version of the same id cannot take an installed plugin over.

use std::collections::BTreeMap;

use serde::Serialize;

use super::{IndexEntry, PluginIndex};
use crate::manifest::SUPPORTED_API_VERSIONS;

/// Whether an entry runs on this app.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum Compatibility {
    /// It runs here.
    Compatible,
    /// It needs a newer rBrowser.
    #[serde(rename_all = "camelCase")]
    NeedsNewerApp { min_app_version: String },
    /// It was built against a contract this app does not run.
    #[serde(rename_all = "camelCase")]
    ApiVersion { api_version: String },
}

impl Compatibility {
    /// Whether the entry can be installed here.
    #[must_use]
    pub fn is_compatible(&self) -> bool {
        matches!(self, Self::Compatible)
    }
}

/// Whether `entry` runs on an app of `app_version`.
#[must_use]
pub fn compatibility(entry: &IndexEntry, app_version: &semver::Version) -> Compatibility {
    if !SUPPORTED_API_VERSIONS.contains(&entry.api_version.as_str()) {
        return Compatibility::ApiVersion {
            api_version: entry.api_version.clone(),
        };
    }
    if let Some(minimum) = &entry.min_app_version
        && semver::Version::parse(minimum)
            .ok()
            .is_none_or(|minimum| *app_version < minimum)
    {
        return Compatibility::NeedsNewerApp {
            min_app_version: minimum.clone(),
        };
    }
    Compatibility::Compatible
}

/// One repository's offer of a plugin: its newest version that runs here, or - when none does -
/// its newest version with the reason.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Offer {
    /// The repository's id (`official`, or the id of a repository the person added).
    pub repository: String,
    pub entry: IndexEntry,
    pub compatibility: Compatibility,
}

/// One plugin id of the catalogue with every repository's offer, in the repositories' order.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogEntry {
    pub id: String,
    pub offers: Vec<Offer>,
}

impl CatalogEntry {
    /// The offer of `repository`.
    #[must_use]
    pub fn offer_of(&self, repository: &str) -> Option<&Offer> {
        self.offers
            .iter()
            .find(|offer| offer.repository == repository)
    }

    /// Whether more than one repository lists this id: the interface names the origins.
    #[must_use]
    pub fn has_several_origins(&self) -> bool {
        self.offers.len() > 1
    }
}

/// The catalogue of `indexes` (repository id and its verified index, in the order the person
/// lists the repositories, the official one first), by plugin id.
#[must_use]
pub fn merge_offers(
    indexes: &[(String, &PluginIndex)],
    app_version: &semver::Version,
) -> Vec<CatalogEntry> {
    let mut by_id: BTreeMap<String, Vec<Offer>> = BTreeMap::new();
    for (repository, index) in indexes {
        let mut newest: BTreeMap<&str, &IndexEntry> = BTreeMap::new();
        for entry in &index.plugins {
            let slot = newest.entry(entry.id.as_str()).or_insert(entry);
            if better(entry, slot, app_version) {
                *slot = entry;
            }
        }
        for (id, entry) in newest {
            by_id.entry(id.to_owned()).or_default().push(Offer {
                repository: repository.clone(),
                entry: entry.clone(),
                compatibility: compatibility(entry, app_version),
            });
        }
    }
    by_id
        .into_iter()
        .map(|(id, offers)| CatalogEntry { id, offers })
        .collect()
}

/// Whether `candidate` beats `held`: a compatible entry over an incompatible one, then the higher
/// version.
fn better(candidate: &IndexEntry, held: &IndexEntry, app_version: &semver::Version) -> bool {
    let runs = |entry: &IndexEntry| compatibility(entry, app_version).is_compatible();
    match (runs(candidate), runs(held)) {
        (true, false) => true,
        (false, true) => false,
        _ => version_of(candidate) > version_of(held),
    }
}

fn version_of(entry: &IndexEntry) -> Option<semver::Version> {
    semver::Version::parse(&entry.version).ok()
}

/// The update of a plugin installed at `installed` from `origin`: that repository's offer when it
/// runs here and is newer. Another repository's higher version is no update.
#[must_use]
pub fn update_for<'a>(
    entry: &'a CatalogEntry,
    origin: &str,
    installed: &semver::Version,
) -> Option<&'a Offer> {
    entry.offer_of(origin).filter(|offer| {
        offer.compatibility.is_compatible()
            && version_of(&offer.entry).is_some_and(|version| version > *installed)
    })
}

#[cfg(test)]
#[path = "index_merge_tests.rs"]
mod tests;
