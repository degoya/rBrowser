//! What the catalogue keeps on disk (RB-091-01), in its folder beside the user settings
//! (`plugin-catalog/`): each repository's last accepted index, the replay floor per repository,
//! and which plugin the catalogue installed from which repository.
//!
//! Adapted from rb-update's `floors.rs` and rDownloader's repository cache. Every file is written
//! whole (`rb_util::fs::write_atomic`), so a crash leaves the old file or the new one. A
//! file that does not read is treated as absent - the floors then start again from the cached
//! indexes, which are verified again at every start.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize, de::DeserializeOwned};

use super::RepositoryError;

/// The replay floors, by repository id.
pub const FLOORS_FILE: &str = "floors.json";
/// The catalogue's installs, by plugin id.
pub const INSTALLS_FILE: &str = "installs.json";

/// One plugin the catalogue installed.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct InstallRecord {
    /// The repository it came from: updates come from there only.
    pub repository: String,
    pub version: String,
    /// SHA-256 of the file written.
    pub sha256: String,
    /// The fingerprint of the key the installed version is signed with: an update signed by
    /// another key is confirmed separately (security review RB-091-01, H1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fingerprint: Option<String>,
}

/// One read-modify-write of the floors or the installs at a time in this process (security review
/// RB-091-01, L5): two refreshes or an install beside a refresh never lose each other's write.
static WRITES: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// The catalogue's folder.
#[derive(Clone, Debug)]
pub struct CatalogStore {
    root: PathBuf,
}

impl CatalogStore {
    /// The store in `root`, created on the first write.
    #[must_use]
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    /// The highest sequence accepted from each repository.
    #[must_use]
    pub fn floors(&self) -> BTreeMap<String, u64> {
        self.read_json(FLOORS_FILE).unwrap_or_default()
    }

    /// Raises the floor of `repository` to `sequence`; never lowers it.
    ///
    /// # Errors
    ///
    /// The file cannot be written.
    pub fn raise_floor(&self, repository: &str, sequence: u64) -> Result<(), RepositoryError> {
        let _one = rb_util::lock(&WRITES);
        let mut floors = self.floors();
        let floor = floors.entry(repository.to_owned()).or_insert(0);
        if sequence > *floor {
            *floor = sequence;
            self.write_json(FLOORS_FILE, &floors)?;
        }
        Ok(())
    }

    /// The cached index bytes of `repository`.
    #[must_use]
    pub fn cached_index(&self, repository: &str) -> Option<Vec<u8>> {
        std::fs::read(self.index_path(repository)).ok()
    }

    /// Caches the accepted index bytes of `repository`.
    ///
    /// # Errors
    ///
    /// The file cannot be written.
    pub fn cache_index(&self, repository: &str, bytes: &[u8]) -> Result<(), RepositoryError> {
        self.write_atomic(&self.index_path(repository), bytes)
    }

    /// Drops the cached index of `repository`, which no longer verifies.
    pub fn drop_index(&self, repository: &str) {
        let _ = std::fs::remove_file(self.index_path(repository));
    }

    /// The plugins the catalogue installed.
    #[must_use]
    pub fn installs(&self) -> BTreeMap<String, InstallRecord> {
        self.read_json(INSTALLS_FILE).unwrap_or_default()
    }

    /// Records (`Some`) or forgets (`None`) the install of `plugin`.
    ///
    /// # Errors
    ///
    /// The file cannot be written.
    pub fn record_install(
        &self,
        plugin: &str,
        record: Option<InstallRecord>,
    ) -> Result<(), RepositoryError> {
        let _one = rb_util::lock(&WRITES);
        let mut installs = self.installs();
        match record {
            Some(record) => installs.insert(plugin.to_owned(), record),
            None => installs.remove(plugin),
        };
        self.write_json(INSTALLS_FILE, &installs)
    }

    fn index_path(&self, repository: &str) -> PathBuf {
        // Ids are `official` or `user-<hex>` (`Repository`), so the name cannot leave the root.
        self.root.join(format!("index-{repository}.json"))
    }

    fn read_json<T: DeserializeOwned>(&self, name: &str) -> Option<T> {
        let bytes = std::fs::read(self.root.join(name)).ok()?;
        serde_json::from_slice(&bytes).ok()
    }

    fn write_json<T: Serialize>(&self, name: &str, value: &T) -> Result<(), RepositoryError> {
        let bytes = serde_json::to_vec_pretty(value)
            .map_err(|error| RepositoryError::Io(error.to_string()))?;
        self.write_atomic(&self.root.join(name), &bytes)
    }

    fn write_atomic(&self, path: &Path, bytes: &[u8]) -> Result<(), RepositoryError> {
        std::fs::create_dir_all(&self.root)?;
        write_atomic(path, bytes)
    }
}

/// Writes `bytes` to `path` whole ([`rb_util::fs::write_atomic`]).
///
/// # Errors
///
/// The file cannot be written or renamed.
pub(crate) fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), RepositoryError> {
    Ok(rb_util::fs::write_atomic(path, bytes)?)
}

#[cfg(test)]
#[path = "repository_store_tests.rs"]
mod tests;
