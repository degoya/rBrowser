//! The publishing half of the index: one entry per verified `.rbplugin` (`rb-plugin-pack index`,
//! `sdk/ci/repository.yml`, `.github/workflows/release-plugins.yml`).
//!
//! Adapted from rDownloader's `crates/rd-plugin-host/src/index_build.rs` (RD-140-01): every field
//! comes from the verified package itself, so an index cannot promise less than the package asks
//! for.

use anyhow::Context as _;

use super::{IndexEntry, Publisher};
use crate::package::VerifiedPackage;
use crate::sign::{decode_public_key, hex_sha256, key_fingerprint};

/// The file name a package is published under: `<id>-<version>.rbplugin`.
#[must_use]
pub fn package_file_name(id: &str, version: &str) -> String {
    format!("{id}-{version}.{}", crate::package::PACKAGE_EXTENSION)
}

/// The index entry of `package`, whose file is `file`, published at `url` (absolute https, or
/// relative to the index).
///
/// # Errors
///
/// The manifest's public key does not decode, or the file is larger than a `u64`.
pub fn describe_package(
    file: &[u8],
    package: &VerifiedPackage,
    url: &str,
    release_notes: Option<String>,
) -> anyhow::Result<IndexEntry> {
    let manifest = &package.manifest;
    let key = decode_public_key(&manifest.public_key).context("the manifest's public_key")?;
    Ok(IndexEntry {
        id: manifest.id.clone(),
        name: manifest.name.clone(),
        version: manifest.version.to_string(),
        api_version: manifest.api_version.clone(),
        kinds: manifest
            .kinds
            .iter()
            .map(|kind| kind.world().to_owned())
            .collect(),
        description: manifest.description.clone(),
        capabilities: manifest
            .grants
            .granted
            .iter()
            .map(|capability| capability.id().to_owned())
            .collect(),
        min_app_version: manifest.metadata.min_app_version.clone(),
        category: None,
        url: url.to_owned(),
        sha256: hex_sha256(file),
        size: u64::try_from(file.len()).context("package size")?,
        publisher: Publisher {
            key_id: manifest.key_id.clone(),
            fingerprint: key_fingerprint(&key),
            author: manifest.metadata.author.clone(),
        },
        release_notes,
    })
}

/// `current` with what `previous` lists for another `rbrowser:plugin` contract carried over
/// (RB-091-04, as rDownloader's index keeps it): per plugin and contract the newest version, so an
/// installation that has not updated yet still finds what it runs. An entry of a contract
/// `current` lists for that plugin is replaced by it; only entries with an absolute address are
/// carried, a relative one would resolve against the new release.
#[must_use]
pub fn carry_over(previous: &[IndexEntry], mut current: Vec<IndexEntry>) -> Vec<IndexEntry> {
    let covered: std::collections::BTreeSet<(String, String)> = current
        .iter()
        .map(|entry| (entry.id.clone(), entry.api_version.clone()))
        .collect();
    let mut kept: std::collections::BTreeMap<(String, String), &IndexEntry> =
        std::collections::BTreeMap::new();
    for entry in previous {
        let key = (entry.id.clone(), entry.api_version.clone());
        let listed = current
            .iter()
            .any(|known| known.id == entry.id && known.version == entry.version);
        if listed || covered.contains(&key) || !entry.url.starts_with("https://") {
            continue;
        }
        let newer = kept.get(&key).is_none_or(|known| {
            match (
                semver::Version::parse(&entry.version),
                semver::Version::parse(&known.version),
            ) {
                (Ok(found), Ok(known)) => found > known,
                _ => false,
            }
        });
        if newer {
            kept.insert(key, entry);
        }
    }
    current.extend(kept.into_values().cloned());
    current
}

#[cfg(test)]
#[path = "index_build_tests.rs"]
mod tests;
