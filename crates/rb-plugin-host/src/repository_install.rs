//! Downloading, installing and removing an offered package (RB-091-01).
//!
//! Adapted from rDownloader's `repository_prepare.rs` and `repository_apply.rs` (RD-140-01). The
//! order: the entry runs here, the download stays within the size the index names and matches
//! its SHA-256, the package is reviewed ([`super::prepare`]: verified, as described, its key's
//! state), and only then [`install`] holds it to what the person confirmed ([`Consent`]): the
//! capabilities the dialog showed and - for a key not trusted yet or another key than the
//! installed version's - the key's fingerprint, confirmed in a step of its own. **A key is never
//! trusted as a side effect** (security review H1). A plugin file the catalogue did not write is
//! never replaced, and an update must be newer (L2). The file goes into the user's plugin folder,
//! where the next start loads it.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

use url::Url;

use super::prepare::Prepared;
use super::store::write_atomic;
use super::{
    CatalogStore, Fetcher, InstallRecord, PACKAGE_TIMEOUT_SECONDS, RepositoryError, app_version,
};
use crate::index::{IndexEntry, compatibility};
use crate::manifest::valid_plugin_id;
use crate::package::PACKAGE_EXTENSION;

/// What the person confirmed.
#[derive(Clone, Debug, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Consent {
    /// The capability ids the dialog showed.
    pub capabilities: Vec<String>,
    /// The publisher key's fingerprint, confirmed in the key's own step - only when the review
    /// asked for it ([`super::KeyReview::needs_confirmation`]).
    #[serde(default)]
    pub key_fingerprint: Option<String>,
}

/// A publisher key the person confirmed: the app pins it in `plugins.trustedKeys`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PinnedKey {
    pub key_id: String,
    pub public_key: String,
    pub fingerprint: String,
}

/// What an install wrote.
#[derive(Clone, Debug)]
pub struct Installed {
    pub id: String,
    pub version: String,
    pub path: PathBuf,
    /// The key to pin, when the person confirmed one not trusted before.
    pub pin: Option<PinnedKey>,
}

/// Downloads the package `entry` describes, from the index at `index_url`: within its size and
/// matching its SHA-256.
///
/// # Errors
///
/// The entry does not run here, the address is not https, the download fails, is larger than
/// the entry says, or is not the file the index names.
pub fn download(
    fetcher: &dyn Fetcher,
    index_url: &Url,
    entry: &IndexEntry,
) -> Result<Vec<u8>, RepositoryError> {
    if !compatibility(entry, &app_version()).is_compatible() {
        return Err(RepositoryError::Incompatible);
    }
    let url = entry
        .resolve_url(index_url)
        .map_err(|_| RepositoryError::InvalidUrl)?;
    let bytes = fetcher.fetch(
        &url,
        entry.size,
        Duration::from_secs(PACKAGE_TIMEOUT_SECONDS),
    )?;
    super::prepare::matches_entry(&bytes, entry)?;
    Ok(bytes)
}

/// Writes the reviewed package `bytes` of `repository`'s `entry` into `user_dir` as
/// `<id>.rbplugin` when `consent` covers it, and records where it came from in `store`.
///
/// # Errors
///
/// See [`RepositoryError`]; nothing is written on any of them.
pub fn install(
    prepared: &Prepared,
    bytes: &[u8],
    repository: &str,
    entry: &IndexEntry,
    consent: &Consent,
    user_dir: &Path,
    store: &CatalogStore,
) -> Result<Installed, RepositoryError> {
    // Again here, so bytes from elsewhere than the review cannot slip in.
    super::prepare::matches_entry(bytes, entry)?;
    let id = prepared.manifest().id.clone();
    let path = user_dir.join(format!("{id}.{PACKAGE_EXTENSION}"));
    match store.installs().get(&id) {
        None if path.exists() => return Err(RepositoryError::WouldOverwrite(id)),
        Some(record) if !newer(&entry.version, &record.version) => {
            return Err(RepositoryError::NotNewer {
                installed: record.version.clone(),
                offered: entry.version.clone(),
            });
        }
        _ => {}
    }
    let confirmed: BTreeSet<&str> = consent.capabilities.iter().map(String::as_str).collect();
    if prepared.capability_ids() != confirmed {
        return Err(RepositoryError::ConsentMismatch);
    }
    let key = prepared.key();
    if key.needs_confirmation()
        && consent.key_fingerprint.as_deref() != Some(key.fingerprint.as_str())
    {
        return Err(match &key.previous_fingerprint {
            Some(previous) if key.changed() => RepositoryError::KeyChanged {
                fingerprint: key.fingerprint.clone(),
                previous: previous.clone(),
            },
            _ => RepositoryError::KeyUntrusted {
                key_id: key.key_id.clone(),
                fingerprint: key.fingerprint.clone(),
            },
        });
    }
    let pin = (!key.trusted).then(|| PinnedKey {
        key_id: key.key_id.clone(),
        public_key: prepared.public_key().to_owned(),
        fingerprint: key.fingerprint.clone(),
    });
    std::fs::create_dir_all(user_dir)?;
    write_atomic(&path, bytes)?;
    store.record_install(
        &id,
        Some(InstallRecord {
            repository: repository.to_owned(),
            version: entry.version.clone(),
            sha256: entry.sha256.clone(),
            fingerprint: Some(key.fingerprint.clone()),
        }),
    )?;
    Ok(Installed {
        id,
        version: entry.version.clone(),
        path,
        pin,
    })
}

/// Whether `offered` is a higher version than `installed`; a version that does not parse is not.
fn newer(offered: &str, installed: &str) -> bool {
    match (
        semver::Version::parse(offered),
        semver::Version::parse(installed),
    ) {
        (Ok(offered), Ok(installed)) => offered > installed,
        _ => false,
    }
}

/// Removes the plugin `id` the catalogue installed from `user_dir`. A plugin the catalogue did
/// not install - bundled, copied in by hand, a project's - is never touched.
///
/// # Errors
///
/// [`RepositoryError::NotInstalledHere`], or the file cannot be removed.
pub fn remove(id: &str, user_dir: &Path, store: &CatalogStore) -> Result<(), RepositoryError> {
    if !valid_plugin_id(id) || !store.installs().contains_key(id) {
        return Err(RepositoryError::NotInstalledHere);
    }
    match std::fs::remove_file(user_dir.join(format!("{id}.{PACKAGE_EXTENSION}"))) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    store.record_install(id, None)
}

#[cfg(test)]
#[path = "repository_install_tests.rs"]
mod tests;
