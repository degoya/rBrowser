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

#[cfg(test)]
#[path = "index_build_tests.rs"]
mod tests;
